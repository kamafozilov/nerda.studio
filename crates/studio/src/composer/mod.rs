//! The session's composer: where a message to the agent is written. A box of
//! growing text over a toolbar, and over it where the session runs.
//!
//! The project and its branch stand over the box. A draft session centers
//! the composer in the card; once the session has seen work, the composer
//! docks to the card's bottom, leaving the space above for the messages.

mod attachments;
mod chips;
mod intake;
mod pasted;
mod pickers;
mod projects;
mod toolbar;

use std::path::{Path, PathBuf};

use gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    ParentElement, Render, SharedString, Styled, Subscription, Window, actions, div, prelude::*,
    px, relative, svg,
};
use theme::{
    COMPOSER_MAX_LINES, COMPOSER_MAX_WIDTH, COMPOSER_MIN_LINES, COMPOSER_RADIUS, CONTROL_SIZE,
    HAIRLINE, ICON_SIZE, SPACE_1, SPACE_2, SPACE_3, TEXT_SMALL, Theme,
};
use workspace::{Head, SessionId};

use attachments::Attachments;
pub(crate) use chips::{chip, chip_label};
pub use projects::OpenProject;

use crate::{
    agents::{Problem, SessionAgent},
    assets::Icon,
    conversation,
    menu::Menu,
    store::Store,
    text_input::{Lines, TextInput, TextInputEvent},
    tooltip::TooltipExt,
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
    attachments: Attachments,
    /// Why the last attempt to attach, to send or to change the agent fell
    /// short, shown under the box until the next edit.
    problem: Option<SharedString>,
    /// The system's file picker is open: a second click must not open another.
    picking: bool,
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
        let subscriptions = vec![
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
            attachments: Attachments::default(),
            problem: None,
            picking: false,
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
        let sent = (self.agent).update(cx, |agent, cx| agent.send(text, &paths, cx));
        match sent {
            Ok(()) => {
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

    /// The box: the text over the toolbar, on the field's fill, its hairline
    /// turning `composer_focus` while the text has focus. A click anywhere in it
    /// puts the cursor in the text.
    fn render_box(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        let input_focus = self.input.focus_handle(cx);
        let focused = input_focus.contains_focused(window, cx);
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
            .border_color(if focused {
                colors.composer_focus
            } else {
                colors.border
            })
            .bg(colors.composer_background)
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

    /// Where the session runs, over the box: the project's pill on the box's
    /// leading edge, and the branch its checkout is on, or the commit, at
    /// the trailing edge. Their content stands in the box's padding.
    fn render_context(
        &self,
        draft: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let colors = Theme::of(cx).colors;
        let store = self.store.read(cx);
        let workspace = store.workspace();
        let project_id = workspace.session(self.session)?.project;
        let project = workspace.project(project_id)?;
        let name: SharedString = workspace.display_name(project_id)?.into();
        let branch = store.head(project.id).map(|head| match head {
            Head::Branch(name) | Head::Detached(name) => SharedString::from(name.clone()),
        });
        let pill = self.render_project_pill(name, draft, window, cx);
        Some(
            div()
                .h(CONTROL_SIZE)
                .mb(SPACE_1)
                .flex()
                .items_center()
                .justify_between()
                .gap(SPACE_3)
                .text_color(colors.text_secondary)
                .child(pill)
                .when_some(branch, |row, branch| {
                    row.child(
                        div()
                            .id("composer-branch")
                            .flex_shrink_0()
                            .min_w_0()
                            .max_w(relative(0.4))
                            .pr(SPACE_3)
                            .flex()
                            .items_center()
                            .gap(SPACE_2)
                            .child(
                                svg()
                                    .path(Icon::GitBranch.path())
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
                                    .child(branch.clone()),
                            )
                            .with_tooltip(branch),
                    )
                }),
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
            .max_w(COMPOSER_MAX_WIDTH)
            .flex()
            .flex_col()
            .children(self.render_context(draft, window, cx))
            .child(self.render_box(window, cx))
            .children(self.render_problem(cx));
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
            .w_full()
            .px(SPACE_3)
            .flex()
            .flex_col()
            .items_center()
            .when(draft, |placement| placement.flex_1().min_h_0())
            .when(!draft, |placement| placement.flex_none().pb(SPACE_3))
            .when(draft, |placement| placement.child(space(DRAFT_RISE)))
            .child(column)
            .when(draft, |placement| placement.child(space(1. - DRAFT_RISE)))
    }
}
