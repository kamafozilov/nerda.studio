//! The cursor's blink: solid for a moment after each move or edit, so typing
//! never hides it, then on and off until the input loses focus. In a window
//! in the background the cursor hides, as the system's fields do, so no
//! field ever shows a cursor that stands still.

use std::{ops::Range, time::Duration};

use gpui::{Context, SharedString, Task, Window};

use super::TextInput;

/// How long the cursor stays on, and then off: the system's rhythm.
const BLINK: Duration = Duration::from_millis(530);

pub(super) struct Blink {
    visible: bool,
    /// What the cursor last stood for: the selection, the text and whether
    /// the input had focus in the active window. A change shows the cursor
    /// and starts over.
    seen: Option<(Range<usize>, SharedString, bool)>,
    _task: Option<Task<()>>,
}

impl Default for Blink {
    fn default() -> Self {
        Self {
            visible: true,
            seen: None,
            _task: None,
        }
    }
}

impl TextInput {
    /// Whether the cursor shows this frame: blinking on, in the active window.
    pub(super) fn cursor_visible(&self, window: &Window) -> bool {
        self.blink.visible && crate::background::active(window)
    }

    /// Run on each render: shows the cursor and restarts the blink when the
    /// selection, the text or the focus changed; stops it without focus or
    /// while the window is in the background, where the cursor hides.
    pub(super) fn keep_blinking(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.focus_handle.is_focused(window) && crate::background::active(window);
        let seen = Some((self.selected_range.clone(), self.content.clone(), focused));
        if self.blink.seen == seen {
            return;
        }
        self.blink.seen = seen;
        self.blink.visible = true;
        self.blink._task = focused.then(|| {
            cx.spawn_in(window, async move |input, cx| {
                loop {
                    cx.background_executor().timer(BLINK).await;
                    let going = input.update_in(cx, |input, window, cx| {
                        if !input.focus_handle.is_focused(window)
                            || !crate::background::active(window)
                        {
                            input.blink.visible = true;
                            return false;
                        }
                        input.blink.visible = !input.blink.visible;
                        cx.notify();
                        true
                    });
                    if !matches!(going, Ok(true)) {
                        break;
                    }
                }
            })
        });
    }
}
