//! What one build leaves on screen for the next: the window's place, the
//! sidebar's state and whether the window was in front, as one line of text.

use gpui::{Bounds, WindowBounds, point, px, size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub bounds: WindowBounds,
    pub sidebar_open: bool,
    /// The window was in front, so the next one takes the front too; otherwise
    /// a reload must not pull focus from the editor.
    pub active: bool,
}

/// `windowed x y width height sidebar active`, with the last two `1` or `0`.
pub(super) fn format(screen: &Screen) -> String {
    let (kind, bounds) = match screen.bounds {
        WindowBounds::Windowed(bounds) => ("windowed", bounds),
        WindowBounds::Maximized(bounds) => ("maximized", bounds),
        WindowBounds::Fullscreen(bounds) => ("fullscreen", bounds),
    };
    format!(
        "{kind} {} {} {} {} {} {}",
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32(),
        u8::from(screen.sidebar_open),
        u8::from(screen.active),
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
    Some(Screen {
        bounds,
        sidebar_open,
        active,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_survives_the_file_and_damage_reads_as_nothing() {
        let screen = Screen {
            bounds: WindowBounds::Maximized(Bounds {
                origin: point(px(-40.5), px(25.)),
                size: size(px(1200.), px(800.)),
            }),
            sidebar_open: false,
            active: true,
        };
        assert_eq!(parse(&format(&screen)), Some(screen));
        assert_eq!(parse("windowed 1 2 3"), None);
        assert_eq!(parse("tiled 1 2 3 4 1 1"), None);
        assert_eq!(parse(""), None);
    }
}
