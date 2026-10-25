//! Settings: not a tab but the whole window. While it is open the sidebar
//! holds its menu (`nav`) and the card the chosen section: its title over its
//! parts, each a heading over a group of rows, in a column centered in the
//! card.
//!
//! Tab and Shift-Tab move through the sections and the controls; Space or
//! Enter picks a section or works a control. Escape clears the search, else
//! leaves Settings, as Back in the menu does. The section chosen stays while
//! the app runs.

mod general;
mod nav;
mod rows;
mod section;

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Subscription, Window, actions, div, point, px,
};
use theme::{
    SETTINGS_CONTENT_MAX_WIDTH, SETTINGS_PART_GAP, SETTINGS_TOP, SPACE_3, TEXT_TITLE, Theme,
};

use self::section::Section;
use crate::{
    menu::Menu,
    sidebar::fade::Fills,
    store::Store,
    text_input::{Lines, TextInput, TextInputEvent},
};

actions!(
    settings,
    [
        /// Moves focus to the next section or control.
        FocusNext,
        /// Moves focus to the previous section or control.
        FocusPrevious,
        /// Clears the search, else leaves Settings.
        Dismiss,
    ]
);

const CONTEXT: &str = "Settings";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        gpui::KeyBinding::new("shift-tab", FocusPrevious, Some(CONTEXT)),
        gpui::KeyBinding::new("escape", Dismiss, Some(CONTEXT)),
    ]);
}

/// Asks the window to leave Settings.
pub struct Leave;

pub struct SettingsPage {
    store: Entity<Store>,
    focus: FocusHandle,
    /// Each section's row in the menu, Tab's first stops.
    rows_focus: Vec<FocusHandle>,
    section: Section,
    search: Entity<TextInput>,
    /// The choice of what the window opens to.
    launch_menu: Menu<Self, ()>,
    scroll: ScrollHandle,
    /// The menu's rows' fills, fading as the sidebar's do.
    fills: Fills,
    /// The menu's row under the pointer.
    hovered: Option<usize>,
    _search: Subscription,
}

impl EventEmitter<Leave> for SettingsPage {}

impl SettingsPage {
    pub fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        let search = cx.new(|cx| {
            TextInput::new("Search settings", Lines::Single, cx).with_quiet_placeholder()
        });
        let subscription = cx.subscribe(&search, |_, _, TextInputEvent::Changed, cx| cx.notify());
        Self {
            store,
            focus: cx.focus_handle(),
            rows_focus: (0..Section::ALL.len())
                .map(|at| cx.focus_handle().tab_index(at as isize).tab_stop(true))
                .collect(),
            section: Section::General,
            search,
            launch_menu: Menu::new(|page| &mut page.launch_menu, cx),
            scroll: ScrollHandle::new(),
            fills: Fills::default(),
            hovered: None,
            _search: subscription,
        }
    }

    #[cfg(all(target_os = "macos", test))]
    pub fn section_title(&self) -> &'static str {
        self.section.title()
    }

    fn query(&self, cx: &App) -> String {
        self.search.read(cx).text().trim().to_owned()
    }

    /// Shows `section`, from its top, and ends a search.
    fn show(&mut self, section: Section, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_text("", cx));
        if self.section != section {
            self.section = section;
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        cx.notify();
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.query(cx).is_empty() {
            cx.emit(Leave);
        } else {
            self.search.update(cx, |search, cx| search.set_text("", cx));
            window.focus(&self.focus, cx);
        }
    }

    /// From the page itself, which is no stop, Tab starts at the first section.
    fn focus_next(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus.is_focused(window) {
            window.focus(&self.rows_focus[0], cx);
        } else {
            window.focus_next(cx);
        }
    }

    /// The keys every part of Settings answers, the menu's and the card's.
    fn keys<E: InteractiveElement>(&self, element: E, cx: &mut Context<Self>) -> E {
        element
            .key_context(CONTEXT)
            .on_action(cx.listener(|page, _: &FocusNext, window, cx| page.focus_next(window, cx)))
            .on_action(cx.listener(|_, _: &FocusPrevious, window, cx| window.focus_prev(cx)))
            .on_action(cx.listener(|page, _: &Dismiss, window, cx| page.dismiss(window, cx)))
    }
}

impl Focusable for SettingsPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// The card: the chosen section's title over its parts, or over what it will
/// hold while it has none yet.
impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let first_control = Section::ALL.len() as isize;
        let parts = match self.section {
            Section::General => self.render_general(first_control, window, cx),
            other => vec![
                div()
                    .text_color(colors.text_secondary)
                    .child(other.purpose())
                    .into_any_element(),
            ],
        };
        let column = div()
            .w_full()
            .max_w(SETTINGS_CONTENT_MAX_WIDTH)
            .px(SPACE_3 * 2.)
            .pt(SETTINGS_TOP)
            .pb(SETTINGS_PART_GAP)
            .flex()
            .flex_col()
            .gap(SETTINGS_PART_GAP)
            .child(
                div()
                    .text_size(TEXT_TITLE)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors.text_primary)
                    .child(self.section.title()),
            )
            .children(parts);
        self.keys(div(), cx)
            .track_focus(&self.focus)
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("settings-section")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(div().w_full().flex().justify_center().child(column)),
            )
    }
}
