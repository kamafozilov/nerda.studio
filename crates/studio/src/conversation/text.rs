//! The agent's text in a row: its Markdown, parsed again only once it
//! grew; the parts of its end still fading in; and its code blocks' colors
//! and Copy.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{AnyElement, ClipboardItem, Context, SharedString, Window};
use theme::Colors;

use super::{Conversation, animation, markdown, render};

/// How long a code block's Copy shows that it copied.
const COPIED_FOR: Duration = Duration::from_millis(1200);

impl Conversation {
    /// Item `item` of turn `turn`, the text `text`, as its blocks.
    pub(super) fn text(
        &mut self,
        turn: usize,
        item: usize,
        text: &str,
        colors: Colors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let blocks = self.markdown(turn, item, text);
        let tail = self.tail(turn, item, text.len());
        let key = super::key(turn, item);
        let folder = self.folder.clone();
        let mut env = self.env(key, colors, window, cx).text(folder, &blocks);
        render::blocks(&blocks, key, &tail, &mut env)
    }

    /// The text of the person's message in turn `turn`, as they wrote it,
    /// selectable as a reply is.
    pub(super) fn message_text(
        &mut self,
        turn: usize,
        text: &str,
        colors: Colors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = super::key(turn, super::MESSAGE);
        let mut env = self.env(key, colors, window, cx);
        render::plain(text, key, &mut env)
    }

    /// What drawing the text whose key is `key` takes, its pieces in the
    /// selection anew.
    fn env<'a>(
        &'a mut self,
        key: u64,
        colors: Colors,
        window: &'a Window,
        cx: &mut Context<Self>,
    ) -> render::Env<'a> {
        let copy = cx.listener(|conversation, (id, text): &(u64, SharedString), _, cx| {
            conversation.copy(*id, text, cx)
        });
        let copied = self.copied.map(|(id, _)| id);
        self.selecting.borrow_mut().redraw(key);
        let menu = cx.listener(|conversation, opened, window, cx| {
            conversation.open_text_menu(opened, window, cx)
        });
        let selecting = render::Selecting {
            state: self.selecting.clone(),
            item: key,
            focus: self.focus.clone(),
            menu: Rc::new(menu),
            next: 0,
            before: "",
        };
        let mut env = render::Env::new(colors, window, &mut self.syntax, copied, Rc::new(copy));
        env.selecting = Some(selecting);
        env
    }

    /// The Markdown of item `item` of turn `turn`, parsed again only once
    /// its text grew.
    fn markdown(&mut self, turn: usize, item: usize, text: &str) -> Rc<Vec<markdown::Block>> {
        match self.parsed.get(&(turn, item)) {
            Some((length, blocks)) if *length == text.len() => blocks.clone(),
            _ => {
                let blocks = Rc::new(markdown::parse(text));
                self.parsed
                    .insert((turn, item), (text.len(), blocks.clone()));
                blocks
            }
        }
    }

    /// Copies code block `id`'s text, and shows its Copy as done for a
    /// moment.
    pub(super) fn copy(&mut self, id: u64, text: &SharedString, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        let now = Instant::now();
        self.copied = Some((id, now));
        cx.notify();
        cx.spawn(async move |conversation, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            conversation
                .update(cx, |conversation, cx| {
                    if conversation.copied == Some((id, now)) {
                        conversation.copied = None;
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
    }

    /// The parts of the end of item `item` of turn `turn`, `length` long now,
    /// still fading in, measured from its end.
    fn tail(&self, turn: usize, item: usize, length: usize) -> Vec<(usize, usize, f32)> {
        let Some(parts) = self.arriving.get(&(turn, item)) else {
            return Vec::new();
        };
        let ends = parts
            .iter()
            .skip(1)
            .map(|(start, _)| *start)
            .chain([length]);
        (parts.iter().zip(ends))
            .map(|(&(start, since), end)| {
                let amount = animation::arrived(since.elapsed());
                (
                    length.saturating_sub(start),
                    length.saturating_sub(end),
                    amount,
                )
            })
            .collect()
    }
}
