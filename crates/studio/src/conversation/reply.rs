//! What the person does with a reply's text: Copy (⌘C) and Select All (⌘A)
//! on its selection, Escape to let it go; a right press opens its menu,
//! which also opens or copies the link or file under the pointer and
//! copies the whole reply; and under a turn that ended, Copy takes
//! everything the agent wrote in it.

use gpui::{
    AnyElement, App, ClipboardItem, Context, ElementId, IntoElement, KeyBinding, ParentElement,
    Pixels, Point, SharedString, Styled, Window, actions, div,
};
use theme::{Colors, SPACE_1};

use agent::{Ending, Item};

use super::{Conversation, MESSAGE, items, render, selection::Target};
use crate::{
    background,
    kbd::Shortcut,
    menu::{Entry, Item as MenuItem, Placement},
};

actions!(
    conversation,
    [
        /// Copies the selected text of a reply.
        Copy,
        /// Selects the whole reply that has the selection, else the last.
        SelectAll,
        /// Lets the selection go.
        Deselect,
    ]
);

pub const CONTEXT: &str = "Conversation";

pub fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    let (copy, all) = if cfg!(target_os = "macos") {
        ("cmd-c", "cmd-a")
    } else {
        ("ctrl-c", "ctrl-a")
    };
    cx.bind_keys([
        KeyBinding::new(copy, Copy, context),
        KeyBinding::new(all, SelectAll, context),
        KeyBinding::new("escape", Deselect, context),
    ]);
}

/// Where the text's menu opened, on which reply, over what.
pub struct TextMenu {
    position: Point<Pixels>,
    item: u64,
    target: Option<Target>,
}

impl Conversation {
    pub(super) fn copy_selection(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selecting.borrow().copied() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub(super) fn select_all(
        &mut self,
        _: &SelectAll,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let item = (self
            .selecting
            .borrow()
            .selection
            .map(|selection| selection.item))
        .or_else(|| self.last_reply(cx));
        if let Some(item) = item {
            self.selecting.borrow_mut().select_all(item);
            window.refresh();
        }
    }

    pub(super) fn deselect(&mut self, _: &Deselect, window: &mut Window, _: &mut Context<Self>) {
        self.selecting.borrow_mut().selection = None;
        window.refresh();
    }

    /// The key of the newest text the agent wrote.
    fn last_reply(&self, cx: &App) -> Option<u64> {
        let turns = self.agent.read(cx).transcript().turns();
        (turns.iter().enumerate().rev()).find_map(|(turn, each)| {
            let item = (each.items.iter()).rposition(|item| matches!(item, Item::Text(_)))?;
            Some(super::key(turn, item))
        })
    }

    /// Opens the text's menu at `position`, over reply `item` and `target`.
    pub(super) fn open_text_menu(
        &mut self,
        (position, item, target): &(Point<Pixels>, u64, Option<Target>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = TextMenu {
            position: *position,
            item: *item,
            target: target.clone(),
        };
        let give_back = Some(self.focus.clone());
        self.text_menu.open_from(menu, give_back, window, cx);
    }

    /// The text's menu, while open.
    pub(super) fn render_text_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.text_menu.opened()?;
        let mut entries: Vec<Entry<Self>> = Vec::new();
        match menu.target.clone() {
            Some(Target::Web(link)) => {
                let copied = link.clone();
                entries.push(Entry::Item(MenuItem::new(
                    "open-link",
                    "Open Link",
                    move |_, _, cx| {
                        if background::allows_system_ui("A link") {
                            cx.open_url(&link);
                        }
                    },
                )));
                entries.push(Entry::Item(MenuItem::new(
                    "copy-link",
                    "Copy Link",
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
                    },
                )));
                entries.push(Entry::Separator);
            }
            Some(Target::File(path)) => {
                let (shown, copied) = (path.clone(), path.clone());
                entries.push(Entry::Item(MenuItem::new(
                    "open-file",
                    "Open",
                    move |_, _, cx| {
                        if background::allows_system_ui("A file") {
                            cx.open_with_system(&path);
                        }
                    },
                )));
                entries.push(Entry::Item(MenuItem::new(
                    "reveal",
                    "Show in Finder",
                    move |_, _, cx| {
                        if background::allows_system_ui("The Finder") {
                            cx.reveal_path(&shown);
                        }
                    },
                )));
                entries.push(Entry::Item(MenuItem::new(
                    "copy-path",
                    "Copy Path",
                    move |_, _, cx| {
                        let text = copied.display().to_string();
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    },
                )));
                entries.push(Entry::Separator);
            }
            None => {}
        }
        let has_selection = self.selecting.borrow().has_selection();
        entries.push(Entry::Item(
            MenuItem::new("copy", "Copy", |this: &mut Self, window, cx| {
                this.copy_selection(&Copy, window, cx)
            })
            .enabled(has_selection)
            .shortcut(Shortcut::key("C").cmd()),
        ));
        let item = menu.item;
        entries.push(Entry::Item(
            MenuItem::new(
                "select-all",
                "Select All",
                move |this: &mut Self, window, _| {
                    this.selecting.borrow_mut().select_all(item);
                    window.refresh();
                },
            )
            .shortcut(Shortcut::key("A").cmd()),
        ));
        entries.push(Entry::Separator);
        let whole = if (item & 0xff_ffff) as usize == MESSAGE {
            "Copy Message"
        } else {
            "Copy Reply"
        };
        entries.push(Entry::Item(MenuItem::new(
            "copy-reply",
            whole,
            move |this: &mut Self, _, cx| {
                if let Some(text) = this.reply_text(item, cx) {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            },
        )));
        let position = menu.position;
        self.text_menu
            .render("text-menu", entries, Placement::At(position), window, cx)
    }

    /// The Markdown of the reply whose key is `item`, or the text of the
    /// person's message.
    fn reply_text(&self, item: u64, cx: &App) -> Option<String> {
        let (turn, at) = ((item >> 24) as usize, (item & 0xff_ffff) as usize);
        let turns = self.agent.read(cx).transcript().turns();
        if at == MESSAGE {
            let text = turns.get(turn)?.message.as_ref()?.text.trim_end();
            return Some(text.to_owned());
        }
        match turns.get(turn)?.items.get(at)? {
            Item::Text(text) => Some(text.text.clone()),
            _ => None,
        }
    }

    /// Everything the agent wrote in turn `turn`, a blank line apart.
    fn turn_text(&self, turn: usize, cx: &App) -> Option<String> {
        let turns = self.agent.read(cx).transcript().turns();
        let texts: Vec<&str> = (turns.get(turn)?.items.iter())
            .filter_map(|item| match item {
                Item::Text(text) if !text.text.trim().is_empty() => Some(text.text.trim()),
                _ => None,
            })
            .collect();
        (!texts.is_empty()).then(|| texts.join("\n\n"))
    }

    /// How turn `turn` ended, led by Copy when the agent wrote something
    /// in it; a finished turn's is Copy alone.
    pub(super) fn end_row(
        &self,
        turn: usize,
        ending: &Ending,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let end = items::end(ending, colors);
        let Some(text) = self.turn_text(turn, cx) else {
            return end.unwrap_or_else(|| div().into_any_element());
        };
        // The turn's own id among the code blocks' and the replies'.
        let id = u64::MAX - turn as u64;
        let copied = self.copied.map(|(copied, _)| copied) == Some(id);
        let text = SharedString::from(text);
        let copy = cx.listener(move |this, _: &(), _, cx| this.copy(id, &text, cx));
        let button = render::copy_button(
            ElementId::NamedInteger("copy-turn".into(), turn as u64),
            copied,
            "Copy reply",
            colors,
            move |window, cx| copy(&(), window, cx),
        );
        div()
            .flex()
            .items_center()
            .gap(SPACE_1)
            .child(button)
            .children(end.map(|end| div().flex_1().min_w_0().child(end)))
            .into_any_element()
    }
}
