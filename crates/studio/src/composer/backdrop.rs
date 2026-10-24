//! A draft's backdrop: the pattern behind its composer, and the button in
//! the card's bottom-right corner whose menu picks the pattern, its color,
//! and whether it moves.

use gpui::{
    Anchor, ClickEvent, Context, Focusable, IntoElement, ParentElement, Styled, Window, div,
    prelude::*,
};
use theme::{CONTROL_RADIUS, SPACE_2, Theme};
use workspace::{Backdrop, BackdropStyle, BackdropTint, Workspace};

use super::Composer;
use crate::{
    backdrop,
    menu::{Entry, Item, Placement},
    sidebar::icon_button,
};

impl Composer {
    /// The chosen pattern behind a draft; none once the session has seen
    /// work, when the app lets go of its image.
    pub(super) fn render_backdrop(
        &self,
        draft: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let chosen = self.store.read(cx).workspace().backdrop();
        let column = self.column.clone();
        let element = draft
            .then(|| backdrop::backdrop(chosen.style, chosen.tint, chosen.moving, column, cx))
            .flatten();
        if element.is_none() {
            backdrop::Backdrop::global(cx).update(cx, |backdrop, _| backdrop.release(window));
        }
        element
    }

    /// The backdrop's button, `SPACE_2` in from the card's bottom-right
    /// corner, on the composer's fill under a hairline; its menu opens above
    /// it.
    pub(super) fn render_backdrop_button(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let open = self.backdrop_menu.opened().is_some();
        let menu = if open {
            let entries = self.backdrop_entries(cx);
            (self.backdrop_menu).render(
                "composer-backdrop-menu",
                entries,
                Placement::Hang(Anchor::BottomRight),
                window,
                cx,
            )
        } else {
            None
        };
        // On the composer's fill, as the project's pill is, so the icon
        // reads the same over any pattern.
        div()
            .absolute()
            .right(SPACE_2)
            .bottom(SPACE_2)
            .rounded(CONTROL_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(colors.composer_background)
            .child(icon_button(
                "composer-backdrop",
                crate::assets::Icon::Backdrop,
                "Background",
                true,
                colors,
                |button| {
                    button
                        .when(open, |button| button.bg(colors.hover))
                        .on_click(cx.listener(|composer, _: &ClickEvent, window, cx| {
                            let text = composer.input.focus_handle(cx);
                            composer.backdrop_menu.open_from((), Some(text), window, cx);
                        }))
                },
            ))
            .children(menu.map(|menu| div().absolute().top_0().right_0().child(menu)))
    }

    /// The patterns, then the colors, then whether it moves; each choice
    /// checked while it applies.
    fn backdrop_entries(&self, cx: &Context<Self>) -> Vec<Entry<Self>> {
        let chosen = self.store.read(cx).workspace().backdrop();
        let mut entries = vec![Entry::Section("Pattern".into())];
        entries.extend(BackdropStyle::ALL.into_iter().map(|style| {
            choice(
                ("backdrop-style", style as u64),
                style.title(),
                chosen.style == style,
                move |backdrop| backdrop.style = style,
            )
        }));
        entries.push(Entry::Section("Color".into()));
        entries.extend(BackdropTint::ALL.into_iter().map(|tint| {
            choice(
                ("backdrop-tint", tint as u64),
                tint.title(),
                chosen.tint == tint,
                move |backdrop| backdrop.tint = tint,
            )
        }));
        entries.push(Entry::Separator);
        entries.push(choice(
            "backdrop-moving",
            "Move slowly",
            chosen.moving,
            |backdrop| backdrop.moving = !backdrop.moving,
        ));
        entries
    }
}

/// An item that changes the backdrop, checked while `checked`.
fn choice(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    checked: bool,
    change: impl Fn(&mut Backdrop) + 'static,
) -> Entry<Composer> {
    Entry::Item(
        Item::new(id, label, move |composer: &mut Composer, _, cx| {
            composer.store.update(cx, |store, cx| {
                store.update(cx, |workspace: &mut Workspace| {
                    let mut backdrop = workspace.backdrop();
                    change(&mut backdrop);
                    workspace.set_backdrop(backdrop);
                })
            })
        })
        .checked(checked),
    )
}
