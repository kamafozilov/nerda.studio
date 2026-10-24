//! The session's composer: where a message to the agent is written. A box of
//! growing text over a toolbar. Under a draft's box hangs a drawer with the
//! project and the branch it will run on (`context`); once the session has
//! seen work they are settled, and the box stands alone. A draft session
//! centers the composer in the card; once the session has seen work, the
//! composer docks to the card's bottom, leaving the space above for the
//! messages.

mod attachments;
mod backdrop;
mod backdrop_cards;
mod backdrop_controls;
mod backdrop_panel;
mod branches;
mod chips;
mod context;
mod dock;
mod intake;
mod pasted;
mod pickers;
mod projects;
mod toolbar;
mod tray;

use std::path::{Path, PathBuf};

use gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, Render, SharedString, Styled, Subscription, Window, actions, div, prelude::*,
    px,
};
use theme::{
    COMPOSER_MAX_LINES, COMPOSER_MAX_WIDTH, COMPOSER_MIN_LINES, COMPOSER_RADIUS,
    CONVERSATION_MAX_WIDTH, HAIRLINE, SPACE_2, SPACE_3, TEXT_SMALL, Theme,
};
use workspace::SessionId;

use attachments::Attachments;
pub(crate) use chips::{chip, chip_label};
pub use projects::OpenProject;

use crate::{
    agents::{Problem, SessionAgent},
    conversation,
    menu::Menu,
    store::Store,
    text_input::{Lines, TextInput, TextInputEvent},
};

actions!(
    composer,
    [
        /// Sends the message written in the composer.
        Send,
    ]
);

const CONTEXT: &str = "Composer";

/// The share of a draft card's free height above its composer: under half,
/// so the composer sits above the middle.
const DRAFT_RISE: f32 = 0.45;

/// Enter sends; Shift-Enter, bound by the text input, breaks the line.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("enter", Send, Some(CONTEXT))]);
    backdrop::bind_keys(cx);
}

pub struct Composer {
    store: Entity<Store>,
    session: SessionId,
    /// The session's agent, which a message goes to.
    agent: Entity<SessionAgent>,
    input: Entity<TextInput>,
    /// The picker open over the toolbar, if one is.
    menu: Menu<Composer, pickers::Picker>,
    /// The project menu, open over or under the project's pill.
    projects: Menu<Composer, ()>,
    /// The branch's chip and its menu.
    branches: branches::BranchPicker,
    /// The backdrop's panel, open over its button.
    backdrop_panel: Option<backdrop::OpenPanel>,
    /// Holds key focus while the backdrop's panel is open.
    panel_focus: FocusHandle,
    /// A draft's tray shows its options.
    tray_open: bool,
    /// While the tray's options go, until their spring settles.
    tray_closing: Option<gpui::Task<()>>,
    attachments: Attachments,
    /// Why the last attempt to attach, to send or to change the agent fell
    /// short, shown under the box until the next edit.
    problem: Option<SharedString>,
    /// The system's file picker is open: a second click must not open another.
    picking: bool,
    /// Where the draft's composer stands, read as it is laid out.
    placement: dock::Placement,
    /// The composer's glide to the card's bottom, while it lasts.
    docking: Option<dock::Docking>,
    _subscriptions: Vec<Subscription>,
}

impl Composer {
    pub fn new(
        store: Entity<Store>,
        session: SessionId,
        agent: Entity<SessionAgent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(
                "Ask anything, or describe a change",
                Lines::Multi {
                    min: COMPOSER_MIN_LINES,
                    max: COMPOSER_MAX_LINES,
                },
                cx,
            )
        });
        let project_search = cx.new(|cx| TextInput::new("Search projects", Lines::Single, cx));
        let (branches, branch_search) = branches::BranchPicker::new(cx);
        let subscriptions = vec![
            branch_search,
            cx.subscribe(
                &project_search,
                |composer, _, TextInputEvent::Changed, cx| composer.projects.query_changed(cx),
            ),
            cx.subscribe(&input, |composer, _, TextInputEvent::Changed, cx| {
                composer.problem = None;
                cx.notify();
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe(&agent, |_, _, cx| cx.notify()),
            // A backdrop image ready, or a moving one's next frame due.
            cx.observe(&crate::backdrop::Backdrop::global(cx), |_, _, cx| {
                cx.notify()
            }),
            cx.subscribe(&agent, |composer, _, Problem(problem), cx| {
                composer.problem = Some(problem.clone());
                cx.notify();
            }),
        ];
        Self {
            store,
            session,
            agent,
            input,
            menu: Menu::new(|composer| &mut composer.menu, cx),
            projects: Menu::with_search(
                |composer| &mut composer.projects,
                project_search.clone(),
                cx,
            ),
            branches,
            backdrop_panel: None,
            panel_focus: cx.focus_handle(),
            tray_open: false,
            tray_closing: None,
            attachments: Attachments::default(),
            problem: None,
            picking: false,
            placement: dock::Placement::default(),
            docking: None,
            _subscriptions: subscriptions,
        }
    }

    /// Whether there is something to send: an attachment, or text that is
    /// not whitespace alone.
    fn has_message(&self, cx: &App) -> bool {
        !self.attachments.is_empty() || !self.input.read(cx).text().trim().is_empty()
    }

    /// Attaches the paths not attached yet, and says so if some found no room.
    fn attach(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let refused = self.attachments.add(paths);
        self.problem = (refused > 0).then(|| attachments::refused_text(refused).into());
        cx.notify();
    }

    fn detach(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.attachments.remove(path);
        self.problem = None;
        cx.notify();
    }

    /// Sends the text, its trailing whitespace trimmed, with the
    /// attachments, and clears both; when the agent can't start, both stay
    /// and the problem line says why.
    fn send(&mut self, _: &Send, _: &mut Window, cx: &mut Context<Self>) {
        if !self.has_message(cx) {
            return;
        }
        let text = self.input.read(cx).text().trim_end().to_owned();
        let paths: Vec<PathBuf> = (self.attachments.iter())
            .map(|attachment| attachment.path.clone())
            .collect();
        let draft = (self.store.read(cx).workspace().session(self.session))
            .is_none_or(|session| session.is_empty());
        let sent = (self.agent).update(cx, |agent, cx| agent.send(text, &paths, cx));
        match sent {
            Ok(()) => {
                if draft {
                    self.start_docking(cx);
                }
                self.input.update(cx, |input, cx| input.set_text("", cx));
                self.attachments = Attachments::default();
                self.problem = None;
            }
            Err(problem) => self.problem = Some(problem),
        }
        cx.notify();
    }

    /// Stops what the agent is doing.
    fn stop(&mut self, cx: &mut Context<Self>) {
        self.agent.update(cx, |agent, _| agent.stop());
    }

    /// The box: the text over the toolbar, on the field's fill under a
    /// `border` hairline that stays as it is while the text has focus: the
    /// cursor shows where typing goes. A click anywhere in it puts the
    /// cursor in the text.
    fn render_box(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let input_focus = self.input.focus_handle(cx);
        let toolbar = self.render_toolbar(window, cx);
        let text = self.render_text(cx);
        let target = div().id("composer-box");
        self.accept_drops(target, cx)
            .w_full()
            .p(SPACE_3 - HAIRLINE)
            .flex()
            .flex_col()
            .gap(SPACE_2)
            .rounded(COMPOSER_RADIUS)
            .border_1()
            .border_color(colors.border)
            .bg(self.surfaces(cx).raised)
            .cursor_text()
            .on_click(
                cx.listener(move |_, _: &ClickEvent, window, cx| window.focus(&input_focus, cx)),
            )
            .children(self.render_chips(cx))
            .child(text)
            .child(toolbar)
    }

    /// Why attaching fell short, under the box and in line with its text.
    fn render_problem(&self, cx: &App) -> Option<impl IntoElement> {
        let problem = self.problem.clone()?;
        Some(
            div()
                .mt(SPACE_2)
                .px(SPACE_3)
                .text_size(TEXT_SMALL)
                .text_color(Theme::of(cx).colors.text_primary)
                .child(conversation::markdown_line(&problem)),
        )
    }
}

impl Focusable for Composer {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let draft = self
            .store
            .read(cx)
            .workspace()
            .session(self.session)
            .is_none_or(|session| session.is_empty());
        let column = div()
            .w_full()
            .max_w(if draft {
                COMPOSER_MAX_WIDTH
            } else {
                CONVERSATION_MAX_WIDTH
            })
            .flex()
            .flex_col()
            .child(self.render_box(window, cx))
            .children(self.render_context(draft, window, cx))
            .children(self.render_problem(cx))
            .when(draft, |column| {
                column.relative().child(self.placement.reader(true))
            });
        let column = if draft {
            column.into_any_element()
        } else {
            self.docked(column)
        };
        let backdrop = self.render_backdrop(draft, window, cx);
        // A draft's column stands a little above the middle, where the eye
        // rests: the space above it is `DRAFT_RISE` of what is left.
        let space = |share: f32| {
            let mut space = div().flex_basis(px(0.)).min_h_0();
            space.style().flex_grow = Some(share);
            space
        };
        div()
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::send))
            .capture_action(cx.listener(Self::paste))
            .relative()
            .w_full()
            .px(SPACE_3)
            .flex()
            .flex_col()
            .items_center()
            .when(draft, |placement| placement.flex_1().min_h_0())
            .when(!draft, |placement| placement.flex_none().pb(SPACE_2))
            // Under everything else, so it paints first.
            .children(backdrop)
            .when(draft, |placement| placement.child(space(DRAFT_RISE)))
            .child(column)
            .when(draft, |placement| placement.child(space(1. - DRAFT_RISE)))
            .when(draft, |placement| {
                placement
                    .child(self.render_tray(window, cx))
                    .child(self.placement.reader(false))
            })
    }
}
