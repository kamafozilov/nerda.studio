//! The takeover's lines over `instance.sock`; the steps are in the parent module.

use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::Arc,
    time::Duration,
};

use gpui::App;

use super::{Previous, hand_off, resume, screen};

/// The request, with the protocol's version.
const HANDOFF: &str = "handoff 1";
/// How long either side waits for a line before counting the other as gone.
const ANSWER_WAIT: Duration = Duration::from_secs(3);
/// How long a new instance gets to draw its window.
const READY_WAIT: Duration = Duration::from_secs(20);

/// Step 1, from the new instance: asks the running one to hand off.
pub(super) fn ask(path: &Path) -> io::Result<Previous> {
    let stream = UnixStream::connect(path)?;
    stream.set_write_timeout(Some(ANSWER_WAIT))?;
    write_line(&stream, HANDOFF)?;
    let answer = read_line(&stream, ANSWER_WAIT)?;
    let screen = answer
        .strip_prefix("handed-off")
        .ok_or_else(|| io::Error::other(format!("the running instance answered {answer:?}")))?;
    Ok(Previous {
        stream,
        screen: screen::parse(screen),
    })
}

/// Listens at `path`, which only the lock's holder may: a crashed instance's
/// leftover file is removed first.
pub(super) fn bind(path: &Path) -> Option<UnixListener> {
    fs::remove_file(path).ok();
    UnixListener::bind(path)
        .inspect_err(|error| {
            eprintln!(
                "can't listen at {}: {error}; a later launch can't take over",
                path.display()
            )
        })
        .ok()
}

/// Answers every launch that connects, until the app quits.
pub(super) fn accept(listener: UnixListener, cx: &mut App) {
    let listener = Arc::new(listener);
    cx.spawn(async move |cx| {
        loop {
            let next = listener.clone();
            // Blocks a background thread until a launch connects.
            match cx
                .background_executor()
                .spawn(async move { next.accept() })
                .await
            {
                Ok((stream, _)) => cx.update(|cx| answer(stream, cx)),
                Err(error) => {
                    eprintln!("stopped listening for later launches: {error}");
                    return;
                }
            }
        }
    })
    .detach();
}

/// Steps 2 and 3, from the running instance.
fn answer(stream: UnixStream, cx: &mut App) {
    cx.spawn(async move |cx| {
        let (stream, request) = read_in_background(cx, stream, ANSWER_WAIT).await;
        if request.as_deref() != Some(HANDOFF) {
            return;
        }
        stream.set_write_timeout(Some(ANSWER_WAIT)).ok();
        let Some(screen) = cx.update(hand_off) else {
            write_line(&stream, "busy").ok();
            return;
        };
        let line = screen.map(|screen| screen::format(&screen));
        let sent = write_line(&stream, &format!("handed-off {}", line.unwrap_or_default()));
        let ready = sent.is_ok()
            && read_in_background(cx, stream, READY_WAIT)
                .await
                .1
                .as_deref()
                == Some("ready");
        cx.update(|cx| if ready { cx.quit() } else { resume(cx) });
    })
    .detach();
}

async fn read_in_background(
    cx: &mut gpui::AsyncApp,
    stream: UnixStream,
    wait: Duration,
) -> (UnixStream, Option<String>) {
    cx.background_executor()
        .spawn(async move {
            let line = read_line(&stream, wait).ok();
            (stream, line)
        })
        .await
}

/// One line, trimmed; an error if the other side closes or `wait` passes first.
fn read_line(stream: &UnixStream, wait: Duration) -> io::Result<String> {
    stream.set_read_timeout(Some(wait))?;
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line)? == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(line.trim().to_owned())
}

pub(super) fn write_line(mut stream: &UnixStream, line: &str) -> io::Result<()> {
    stream.write_all(format!("{line}\n").as_bytes())
}
