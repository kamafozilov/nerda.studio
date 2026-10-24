//! Where the window is, as one line of text: what an instance hands the one
//! taking over from it, and what `window.txt` in the data folder keeps for the
//! next launch.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use gpui::{App, Bounds, DisplayId, WindowBounds, point, px, size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    /// Relative to `display`'s top left corner.
    pub bounds: WindowBounds,
    /// The display's UUID; `None` from an instance that didn't say, whose
    /// bounds count from the primary display.
    pub display: Option<u128>,
    pub sidebar_open: bool,
    /// The window was in front, so the next one takes the front too; otherwise
    /// a takeover must not pull focus from the editor.
    pub active: bool,
    /// The window's number with the window server, for the next window to
    /// open right above it.
    pub window: Option<isize>,
}

impl Screen {
    /// Where to open a window here: the display the window was on, or `None`
    /// once that display is gone, as its bounds then point nowhere.
    pub fn place(&self, cx: &App) -> Option<(WindowBounds, Option<DisplayId>)> {
        let Some(uuid) = self.display else {
            return Some((self.bounds, None));
        };
        let display = cx
            .displays()
            .into_iter()
            .find(|display| display.uuid().is_ok_and(|id| id.as_u128() == uuid))?;
        Some((self.bounds, Some(display.id())))
    }
}

/// `windowed x y width height sidebar active display window`: `sidebar` and
/// `active` are `1` or `0`, `display` a UUID in hex and `window` a number,
/// each `-` when unknown. Older instances stop after `active`.
#[cfg_attr(not(unix), allow(dead_code))]
pub(super) fn format(screen: &Screen) -> String {
    let (kind, bounds) = match screen.bounds {
        WindowBounds::Windowed(bounds) => ("windowed", bounds),
        WindowBounds::Maximized(bounds) => ("maximized", bounds),
        WindowBounds::Fullscreen(bounds) => ("fullscreen", bounds),
    };
    format!(
        "{kind} {} {} {} {} {} {} {} {}",
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32(),
        u8::from(screen.sidebar_open),
        u8::from(screen.active),
        screen
            .display
            .map_or_else(|| "-".to_owned(), |uuid| format!("{uuid:032x}")),
        screen
            .window
            .map_or_else(|| "-".to_owned(), |number| number.to_string()),
    )
}

pub(super) fn parse(text: &str) -> Option<Screen> {
    let mut parts = text.split_whitespace();
    let kind = parts.next()?;
    let mut number = || parts.next()?.parse::<f32>().ok();
    let bounds = Bounds {
        origin: point(px(number()?), px(number()?)),
        size: size(px(number()?), px(number()?)),
    };
    let sidebar_open = number()? != 0.;
    let active = number()? != 0.;
    let bounds = match kind {
        "windowed" => WindowBounds::Windowed(bounds),
        "maximized" => WindowBounds::Maximized(bounds),
        "fullscreen" => WindowBounds::Fullscreen(bounds),
        _ => return None,
    };
    let display = match parts.next() {
        None | Some("-") => None,
        Some(hex) => Some(u128::from_str_radix(hex, 16).ok()?),
    };
    let window = match parts.next() {
        None | Some("-") => None,
        Some(number) => Some(number.parse().ok()?),
    };
    Some(Screen {
        bounds,
        display,
        sidebar_open,
        active,
        window,
    })
}

/// The file's format; a file of a later one is left as it is.
const VERSION: u32 = 1;

/// What `window.txt` held at launch, which decides whether it may be written.
#[derive(Debug, PartialEq)]
pub(super) enum Saved {
    /// Missing, or set aside as corrupt: written at quit.
    Nothing,
    Read(Screen),
    /// From a newer build, or unreadable: never written.
    Kept,
}

impl Saved {
    pub(super) fn screen(&self) -> Option<Screen> {
        match self {
            Saved::Read(screen) => Some(*screen),
            Saved::Nothing | Saved::Kept => None,
        }
    }

    pub(super) fn writable(&self) -> bool {
        !matches!(self, Saved::Kept)
    }
}

pub(super) fn read(path: &Path) -> Saved {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Saved::Nothing,
        Err(error) => {
            eprintln!("can't read {}: {error}", path.display());
            return Saved::Kept;
        }
    };
    let (version, line) = text.trim().split_once(' ').unwrap_or((text.trim(), ""));
    match version.parse::<u32>() {
        Ok(VERSION) => {
            if let Some(screen) = parse(line) {
                return Saved::Read(screen);
            }
        }
        Ok(newer) if newer > VERSION => return Saved::Kept,
        _ => {}
    }
    // Corrupt: moved aside, never overwritten, so it can be looked at.
    let aside = aside(path);
    match fs::rename(path, &aside) {
        Ok(()) => {
            eprintln!(
                "{} couldn't be read; set it aside as {}",
                path.display(),
                aside.display()
            );
            Saved::Nothing
        }
        Err(error) => {
            eprintln!("{} couldn't be read or set aside: {error}", path.display());
            Saved::Kept
        }
    }
}

/// Writes `screen` over the file, leaving out the window's number, which
/// means nothing once the window is gone.
pub(super) fn write(path: &Path, screen: &Screen) {
    let screen = Screen {
        window: None,
        ..*screen
    };
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let written = fs::write(&temp, format!("{VERSION} {}\n", format(&screen)))
        .and_then(|()| fs::rename(&temp, path));
    if let Err(error) = written {
        fs::remove_file(&temp).ok();
        eprintln!(
            "can't save the window's place in {}: {error}",
            path.display()
        );
    }
}

fn aside(path: &Path) -> PathBuf {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    path.with_extension(format!("corrupt-{seconds}.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        Screen {
            bounds: WindowBounds::Maximized(Bounds {
                origin: point(px(-40.5), px(25.)),
                size: size(px(1200.), px(800.)),
            }),
            display: Some(0x0123_4567_89ab_cdef_0011_2233_4455_6677),
            sidebar_open: false,
            active: true,
            window: Some(16791),
        }
    }

    #[test]
    fn the_screen_survives_the_line_and_damage_reads_as_nothing() {
        assert_eq!(parse(&format(&screen())), Some(screen()));
        assert_eq!(parse("windowed 1 2 3"), None);
        assert_eq!(parse("tiled 1 2 3 4 1 1"), None);
        assert_eq!(parse("windowed 1 2 3 4 1 1 nothex -"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn an_older_instance_names_no_display_or_window() {
        let older = parse("windowed 1 2 3 4 1 0").unwrap();
        assert_eq!((older.display, older.window), (None, None));
        let unknown = parse("windowed 1 2 3 4 1 0 - -").unwrap();
        assert_eq!((unknown.display, unknown.window), (None, None));
    }

    #[test]
    fn the_file_keeps_the_place_and_guards_what_it_cannot_read() {
        let dir = std::env::temp_dir().join(format!("nerda-window-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("window.txt");

        assert_eq!(read(&path), Saved::Nothing);
        write(&path, &screen());
        let kept = Screen {
            window: None,
            ..screen()
        };
        assert_eq!(read(&path), Saved::Read(kept));

        fs::write(&path, "2 something newer\n").unwrap();
        assert_eq!(read(&path), Saved::Kept);
        assert_eq!(fs::read_to_string(&path).unwrap(), "2 something newer\n");

        fs::write(&path, "garbage").unwrap();
        assert_eq!(read(&path), Saved::Nothing);
        assert!(!path.exists());
        let aside = fs::read_dir(&dir).unwrap().flatten().find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("window.corrupt-")
        });
        assert_eq!(
            fs::read_to_string(aside.unwrap().path()).unwrap(),
            "garbage"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
