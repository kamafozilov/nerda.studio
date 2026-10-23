//! The attachments as one wrapping row of chips over the text: each its kind's
//! icon and its name, a tooltip with the whole path, and a remove button that
//! shows under the pointer.

use std::{path::Path, sync::Arc};

use gpui::{
    ClickEvent, Context, Div, ElementId, IntoElement, ParentElement, SharedString, Stateful,
    Styled, div, prelude::*, svg,
};
use theme::{
    CHIP_MAX_WIDTH, CHIPS_MAX_HEIGHT, CONTROL_RADIUS, CONTROL_SIZE, Colors, FIELD_TARGET_SIZE,
    ICON_SIZE, ICON_SIZE_SMALL, SPACE_1, SPACE_2, TEXT_SMALL, Theme,
};

use super::{
    Composer,
    attachments::{Attachment, AttachmentKind},
};
use crate::{assets::Icon, tooltip::TooltipExt};

impl Composer {
    /// The row of chips, or nothing while there are no attachments.
    pub(super) fn render_chips(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if self.attachments.is_empty() {
            return None;
        }
        let colors = Theme::of(cx).colors;
        let chips: Vec<_> = self
            .attachments
            .iter()
            .map(|attachment| self.render_chip(attachment, colors, cx))
            .collect();
        Some(
            div()
                .id("composer-chips")
                .max_h(CHIPS_MAX_HEIGHT)
                .overflow_y_scroll()
                .flex()
                .flex_wrap()
                .gap(SPACE_1)
                .cursor_default()
                .children(chips),
        )
    }

    fn render_chip(
        &self,
        attachment: &Attachment,
        colors: Colors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let path: Arc<Path> = attachment.path.as_path().into();
        let group = SharedString::from(format!("composer-chip-{}", path.display()));
        let icon = match attachment.kind {
            AttachmentKind::Image => Icon::Image,
            AttachmentKind::File => Icon::File,
            AttachmentKind::Folder => Icon::Folder,
        };
        let label = chip_label(
            ElementId::Path(path.clone()),
            icon,
            attachment.name().into(),
            colors,
        )
        .with_tooltip(workspace::home_relative(&path));
        let remove_group = SharedString::from(format!("{group}-remove"));
        // 16 visible, taking clicks over WCAG's 24 minimum; the negative
        // margin keeps it where the chip's padding puts it.
        let remove = div()
            .id(ElementId::NamedChild(
                Arc::new(ElementId::Path(path.clone())),
                "remove".into(),
            ))
            .group(remove_group.clone())
            .flex_none()
            .size(FIELD_TARGET_SIZE)
            .m(-(FIELD_TARGET_SIZE - ICON_SIZE) / 2.)
            .flex()
            .items_center()
            .justify_center()
            .invisible()
            .group_hover(group.clone(), |button| button.visible())
            .on_click(cx.listener(move |composer, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                composer.detach(&path, cx);
            }))
            .child(
                div()
                    .size(ICON_SIZE)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(CONTROL_RADIUS)
                    .group_hover(remove_group.clone(), |button| button.bg(colors.hover))
                    .child(
                        svg()
                            .path(Icon::Close.path())
                            .size(ICON_SIZE_SMALL)
                            .text_color(colors.text_secondary)
                            .group_hover(remove_group, |icon| icon.text_color(colors.text_primary)),
                    ),
            )
            .with_tooltip("Remove");
        chip(colors)
            .group(group)
            .pr(SPACE_1)
            .child(label)
            .child(remove)
    }
}

/// A chip without its contents: `CONTROL_SIZE` high on `chip_background`,
/// at most `CHIP_MAX_WIDTH`, its name in `TEXT_SMALL` `text_primary`.
pub(crate) fn chip(colors: Colors) -> Div {
    div()
        .flex_none()
        .max_w(CHIP_MAX_WIDTH)
        .h(CONTROL_SIZE)
        .pl(SPACE_2)
        .flex()
        .items_center()
        .gap(SPACE_1)
        .rounded(CONTROL_RADIUS)
        .bg(colors.chip_background)
        .text_size(TEXT_SMALL)
        .text_color(colors.text_primary)
}

/// A chip's kind, as its icon in `text_secondary`, then its name, cut in the
/// middle so the extension stays.
pub(crate) fn chip_label(
    id: ElementId,
    icon: Icon,
    name: SharedString,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .gap(SPACE_1)
        .child(
            svg()
                .path(icon.path())
                .flex_none()
                .size(ICON_SIZE)
                .text_color(colors.text_secondary),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_middle()
                .child(name),
        )
}
