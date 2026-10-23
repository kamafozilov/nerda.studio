//! Each session's events, one JSON line each in the order they came, in
//! `<data folder>/sessions/<conversation id>.jsonl`, read back to show the
//! conversation after a restart. The file is named by the conversation, which
//! no other workspace shares, rather than by the session, whose id a fresh
//! workspace hands out again. Lines are only ever appended, on a thread of
//! their own; one that doesn't read back is skipped and counted.

use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, ErrorKind, Read, Seek, SeekFrom, Write as _},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use agent::{ConversationId, Event};

/// The thread that appends every session's lines, in order.
#[derive(Clone)]
pub struct Journals {
    folder: Arc<Path>,
    writes: mpsc::Sender<Write>,
    /// A line couldn't be written since this was last taken.
    failed: Arc<AtomicBool>,
}

/// One session's journal.
#[derive(Clone)]
pub struct Journal {
    path: Arc<Path>,
    writes: mpsc::Sender<Write>,
}

enum Write {
    Append(Arc<Path>, Box<Event>),
    /// Answered once every line before it is on disk.
    Flush(mpsc::Sender<()>),
}

/// What a journal held, and what of it couldn't be read.
pub struct Replayed {
    pub events: Vec<Event>,
    pub lost: Lost,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lost {
    #[default]
    Nothing,
    /// This many lines didn't read as events, as when a crash cut the last
    /// one short or a newer build wrote them.
    Lines(usize),
    /// The file is there but couldn't be read.
    All,
}

impl Journals {
    /// Journals in `<data_dir>/sessions`, written by a thread started here.
    pub fn start(data_dir: &Path) -> Self {
        let (writes, inbox) = mpsc::channel();
        let failed = Arc::new(AtomicBool::new(false));
        let flag = failed.clone();
        std::thread::Builder::new()
            .name("journal".to_owned())
            .spawn(move || write_all(&inbox, &flag))
            .expect("the journal's thread starts");
        Self {
            folder: data_dir.join("sessions").into(),
            writes,
            failed,
        }
    }

    pub fn of(&self, conversation: &ConversationId) -> Journal {
        Journal {
            path: self.folder.join(format!("{conversation}.jsonl")).into(),
            writes: self.writes.clone(),
        }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Whether a line failed to be written since the last call.
    pub fn take_failure(&self) -> bool {
        self.failed.swap(false, Ordering::Relaxed)
    }

    /// Waits up to `within` for every line sent so far to be written.
    pub fn flush(&self, within: Duration) {
        let (done, wait) = mpsc::channel();
        if self.writes.send(Write::Flush(done)).is_ok() {
            let _ = wait.recv_timeout(within);
        }
    }
}

impl Journal {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, event: Event) {
        let _ = self
            .writes
            .send(Write::Append(self.path.clone(), Box::new(event)));
    }
}

fn write_all(inbox: &mpsc::Receiver<Write>, failed: &AtomicBool) {
    let mut files: HashMap<Arc<Path>, File> = HashMap::new();
    for write in inbox {
        match write {
            Write::Append(path, event) => {
                let Ok(mut line) = serde_json::to_vec(&event) else {
                    continue;
                };
                line.push(b'\n');
                let written = match files.get_mut(&path) {
                    Some(file) => file.write_all(&line),
                    None => open(&path).and_then(|mut file| {
                        file.write_all(&line)?;
                        files.insert(path.clone(), file);
                        Ok(())
                    }),
                };
                if written.is_err() {
                    // Opened again for the next line, which then starts on
                    // a line of its own.
                    files.remove(&path);
                    failed.store(true, Ordering::Relaxed);
                }
            }
            Write::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}

/// Opens `path` to append to, its folder made if missing. A file whose last
/// line was cut short gets a line break first, so the next line reads.
fn open(path: &Path) -> io::Result<File> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)?;
    if file.seek(SeekFrom::End(0))? > 0 {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0];
        file.read_exact(&mut last)?;
        if last != *b"\n" {
            file.write_all(b"\n")?;
        }
    }
    Ok(file)
}

/// The events the journal at `path` holds; `None` when there is none.
pub fn read(path: &Path) -> Option<Replayed> {
    let text = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(_) => {
            return Some(Replayed {
                events: Vec::new(),
                lost: Lost::All,
            });
        }
    };
    let mut events = Vec::new();
    let mut skipped = 0;
    for line in text.split(|&byte| byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice(line) {
            Ok(event) => events.push(event),
            Err(_) => skipped += 1,
        }
    }
    let lost = match skipped {
        0 => Lost::Nothing,
        lines => Lost::Lines(lines),
    };
    Some(Replayed { events, lost })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nerda-journal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn events_read_back_in_order_and_a_bad_line_is_skipped_and_counted() {
        let dir = folder("order");
        let journals = Journals::start(&dir);
        let journal = journals.of(&ConversationId::new());
        std::fs::create_dir_all(journals.folder()).unwrap();
        let mut earlier = serde_json::to_vec(&Event::Started).unwrap();
        // A line a newer build wrote, then one a crash cut short.
        earlier.extend(b"\n{\"Newer\": 1}\n{\"Exited\":{\"co");
        std::fs::write(journal.path(), earlier).unwrap();
        journal.append(Event::Stderr("oʻzbek и кириллица".to_owned()));
        journal.append(Event::Exited { code: Some(0) });
        journals.flush(Duration::from_secs(5));

        let replayed = read(journal.path()).unwrap();
        assert_eq!(
            replayed.events,
            [
                Event::Started,
                Event::Stderr("oʻzbek и кириллица".to_owned()),
                Event::Exited { code: Some(0) },
            ],
            "the lines after the cut one still read"
        );
        assert_eq!(replayed.lost, Lost::Lines(2));
        assert!(!journals.take_failure());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_journal_reads_as_none() {
        assert!(read(&folder("missing").join("none.jsonl")).is_none());
    }

    #[test]
    fn a_folder_that_cant_be_made_is_reported() {
        let dir = folder("blocked");
        std::fs::create_dir_all(&dir).unwrap();
        // A file where the sessions folder should be.
        std::fs::write(dir.join("sessions"), "").unwrap();
        let journals = Journals::start(&dir);
        journals.of(&ConversationId::new()).append(Event::Started);
        journals.flush(Duration::from_secs(5));
        assert!(journals.take_failure());
        assert!(!journals.take_failure(), "reported once");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
