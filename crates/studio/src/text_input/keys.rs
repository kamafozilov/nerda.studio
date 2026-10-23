//! The input's actions and their platform key bindings.

use gpui::{App, KeyBinding, actions};

actions!(
    text_input,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteToStart,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Home,
        End,
        SelectToStart,
        SelectToEnd,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        /// To the row above; from the first row, to the start.
        Up,
        /// To the row below; from the last row, to the end.
        Down,
        SelectUp,
        SelectDown,
        /// To the start of the whole text, past every row.
        TextStart,
        /// To the end of the whole text, past every row.
        TextEnd,
        SelectToTextStart,
        SelectToTextEnd,
        InsertNewline,
    ]
);

const CONTEXT: Option<&str> = Some("TextInput");
/// A `Lines::Multi` input, which adds rows to move between and breaks to type.
const MULTI: Option<&str> = Some("MultiLine");

/// Registers the input's bindings in key context `TextInput`. Enter and escape
/// stay unbound so the parent can handle them, and so do up and down on one
/// line. Home and End, and their Cmd arrows on macOS, keep to a row in a
/// multi-line input.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", Up, MULTI),
        KeyBinding::new("down", Down, MULTI),
        KeyBinding::new("shift-up", SelectUp, MULTI),
        KeyBinding::new("shift-down", SelectDown, MULTI),
        KeyBinding::new("shift-enter", InsertNewline, MULTI),
    ]);
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("cmd-up", TextStart, MULTI),
            KeyBinding::new("cmd-down", TextEnd, MULTI),
            KeyBinding::new("cmd-shift-up", SelectToTextStart, MULTI),
            KeyBinding::new("cmd-shift-down", SelectToTextEnd, MULTI),
        ]);
    } else {
        cx.bind_keys([
            KeyBinding::new("ctrl-home", TextStart, MULTI),
            KeyBinding::new("ctrl-end", TextEnd, MULTI),
            KeyBinding::new("ctrl-shift-home", SelectToTextStart, MULTI),
            KeyBinding::new("ctrl-shift-end", SelectToTextEnd, MULTI),
        ]);
    }
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, CONTEXT),
        KeyBinding::new("shift-backspace", Backspace, CONTEXT),
        KeyBinding::new("delete", Delete, CONTEXT),
        KeyBinding::new("left", Left, CONTEXT),
        KeyBinding::new("right", Right, CONTEXT),
        KeyBinding::new("shift-left", SelectLeft, CONTEXT),
        KeyBinding::new("shift-right", SelectRight, CONTEXT),
        KeyBinding::new("home", Home, CONTEXT),
        KeyBinding::new("end", End, CONTEXT),
        KeyBinding::new("shift-home", SelectToStart, CONTEXT),
        KeyBinding::new("shift-end", SelectToEnd, CONTEXT),
    ]);
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("alt-backspace", DeleteWordLeft, CONTEXT),
            KeyBinding::new("cmd-backspace", DeleteToStart, CONTEXT),
            KeyBinding::new("alt-left", WordLeft, CONTEXT),
            KeyBinding::new("alt-right", WordRight, CONTEXT),
            KeyBinding::new("alt-shift-left", SelectWordLeft, CONTEXT),
            KeyBinding::new("alt-shift-right", SelectWordRight, CONTEXT),
            KeyBinding::new("cmd-left", Home, CONTEXT),
            KeyBinding::new("cmd-right", End, CONTEXT),
            KeyBinding::new("cmd-shift-left", SelectToStart, CONTEXT),
            KeyBinding::new("cmd-shift-right", SelectToEnd, CONTEXT),
            KeyBinding::new("cmd-a", SelectAll, CONTEXT),
            KeyBinding::new("cmd-c", Copy, CONTEXT),
            KeyBinding::new("cmd-x", Cut, CONTEXT),
            KeyBinding::new("cmd-v", Paste, CONTEXT),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, CONTEXT),
        ]);
    } else {
        cx.bind_keys([
            KeyBinding::new("ctrl-backspace", DeleteWordLeft, CONTEXT),
            KeyBinding::new("ctrl-left", WordLeft, CONTEXT),
            KeyBinding::new("ctrl-right", WordRight, CONTEXT),
            KeyBinding::new("ctrl-shift-left", SelectWordLeft, CONTEXT),
            KeyBinding::new("ctrl-shift-right", SelectWordRight, CONTEXT),
            KeyBinding::new("ctrl-a", SelectAll, CONTEXT),
            KeyBinding::new("ctrl-c", Copy, CONTEXT),
            KeyBinding::new("ctrl-x", Cut, CONTEXT),
            KeyBinding::new("ctrl-v", Paste, CONTEXT),
            KeyBinding::new("shift-delete", Cut, CONTEXT),
            KeyBinding::new("ctrl-insert", Copy, CONTEXT),
            KeyBinding::new("shift-insert", Paste, CONTEXT),
        ]);
    }
}
