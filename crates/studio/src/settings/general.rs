//! General: Startup, what the window opens to; Quitting, whether Cmd-Q asks
//! for a second press.

use gpui::{
    Anchor, ClickEvent, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, svg,
};
use theme::{
    CONTROL_RADIUS, CONTROL_SIZE, ICON_SIZE_SMALL, SETTINGS_PICKER_WIDTH, SPACE_2, TEXT_SMALL,
    Theme,
};
use workspace::{OnLaunch, Workspace};

use super::{
    SettingsPage,
    rows::{Row, part},
    section::Setting,
};
use crate::{
    assets::Icon,
    menu::{Entry, Item, Placement},
    switch::switch,
};

const ON_LAUNCH: Setting = Setting {
    label: "On launch",
    detail: "Open the tabs from last time, or a new session alone",
};

const ASK_BEFORE_QUITTING: Setting = Setting {
    label: "Ask before quitting",
    detail: if cfg!(target_os = "macos") {
        "\u{2318}Q quits on a second press. Quit in the menu always quits at once"
    } else {
        "Ctrl+Q quits on a second press. Quit in the menu always quits at once"
    },
};

/// What the search finds in General.
pub(super) const SETTINGS: [Setting; 2] = [ON_LAUNCH, ASK_BEFORE_QUITTING];

/// The choices of what the window opens to, in the menu's order.
const LAUNCH_CHOICES: [(&str, &str, OnLaunch); 2] = [
    ("on-launch-last-tabs", "Last tabs", OnLaunch::LastTabs),
    ("on-launch-new-session", "New session", OnLaunch::NewSession),
];

fn launch_label(on_launch: OnLaunch) -> &'static str {
    (LAUNCH_CHOICES.iter())
        .find(|(_, _, each)| *each == on_launch)
        .map_or("", |(_, label, _)| *label)
}

impl SettingsPage {
    /// The section's parts; their controls take Tab stops from `first_tab` on.
    pub(super) fn render_general(
        &mut self,
        first_tab: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let preferences = self.store.read(cx).workspace().preferences().clone();
        let confirm_quit = preferences.confirm_quit;
        let launch = Row {
            label: ON_LAUNCH.label.into(),
            detail: ON_LAUNCH.detail.into(),
            control: self
                .render_launch_picker(preferences.on_launch, first_tab, window, cx)
                .into_any_element(),
        };
        let quit = Row {
            label: ASK_BEFORE_QUITTING.label.into(),
            detail: ASK_BEFORE_QUITTING.detail.into(),
            control: switch(
                "settings-confirm-quit",
                confirm_quit,
                first_tab + 1,
                cx.listener(move |page, _: &ClickEvent, _, cx| {
                    page.change(cx, |workspace| workspace.set_confirm_quit(!confirm_quit))
                }),
                cx,
            )
            .into_any_element(),
        };
        vec![
            part("Startup", vec![launch], cx).into_any_element(),
            part("Quitting", vec![quit], cx).into_any_element(),
        ]
    }

    fn change(&self, cx: &mut Context<Self>, change: impl FnOnce(&mut Workspace)) {
        self.store.update(cx, |store, cx| store.update(cx, change));
    }

    /// A button naming the choice, as wide as every picker on the page, whose
    /// menu hangs from its trailing edge.
    fn render_launch_picker(
        &self,
        on_launch: OnLaunch,
        tab_index: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let entries = LAUNCH_CHOICES
            .into_iter()
            .map(|(id, label, choice)| {
                Entry::Item(
                    Item::new(id, label, move |page: &mut SettingsPage, _, cx| {
                        page.change(cx, |workspace| workspace.set_on_launch(choice))
                    })
                    .checked(choice == on_launch),
                )
            })
            .collect();
        let menu = self.launch_menu.render(
            "settings-on-launch-menu",
            entries,
            Placement::Hang(Anchor::TopRight),
            window,
            cx,
        );
        div()
            .relative()
            .child(
                div()
                    .id("settings-on-launch")
                    .tab_index(tab_index)
                    .w(SETTINGS_PICKER_WIDTH)
                    .h(CONTROL_SIZE)
                    .px(SPACE_2)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(SPACE_2)
                    .rounded(CONTROL_RADIUS)
                    .border_1()
                    .border_color(colors.popover_border)
                    .bg(colors.panel_background)
                    .focus_visible(|button| button.border_color(colors.focus_ring))
                    .text_size(TEXT_SMALL)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors.text_primary)
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .child(div().min_w_0().truncate().child(launch_label(on_launch)))
                    .child(
                        svg()
                            .path(Icon::ChevronDown.path())
                            .size(ICON_SIZE_SMALL)
                            .flex_none()
                            .text_color(colors.text_secondary),
                    )
                    .on_click(cx.listener(|page, _: &ClickEvent, window, cx| {
                        page.launch_menu.open((), window, cx)
                    })),
            )
            .children(menu.map(|menu| div().absolute().top_full().right_0().child(menu)))
    }
}
