//! One composer and one conversation per open session, so a message half
//! written, and where the conversation is scrolled, survive switching tabs. Showing a session's tab puts the cursor in its composer,
//! unless a palette, a menu, the quit dialog or the search holds it.

use gpui::{App, AppContext, Context, Entity, Focusable, Subscription, Window};
use workspace::{SessionId, Tab};

use super::Shell;
use crate::{
    agents,
    composer::{Composer, OpenProject},
    conversation::Conversation,
};

pub(super) struct OpenComposer {
    pub composer: Entity<Composer>,
    pub conversation: Entity<Conversation>,
    /// Sends focus on to the composer on screen, should it come back here
    /// while this session is off screen, as when a palette closes.
    _focus: Subscription,
    /// Opens the draft of the project picked over the composer.
    _open_project: Subscription,
}

impl Shell {
    /// Keeps a composer for each open session tab and none for a closed one,
    /// and focuses the one on screen when another tab was shown before.
    pub(super) fn sync_composers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.store.read(cx).workspace();
        let shown = workspace.selected_tab();
        let open: Vec<SessionId> = workspace
            .tabs()
            .iter()
            .filter_map(|tab| tab.session())
            .collect();
        let focus_free = self.focus_is_free(window, cx);
        self.composers.retain(|session, _| open.contains(session));
        for session in open {
            if !self.composers.contains_key(&session) {
                let composer = self.open_composer(session, window, cx);
                self.composers.insert(session, composer);
            }
        }
        if std::mem::replace(&mut self.tab_shown, shown) != shown && focus_free {
            self.focus_shown(window, cx);
        }
    }

    fn open_composer(
        &mut self,
        session: SessionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> OpenComposer {
        let store = self.store.clone();
        let agent = agents::of(session, cx);
        let conversation = cx.new(|cx| Conversation::new(agent.clone(), cx));
        let composer = cx.new(|cx| Composer::new(store, session, agent, cx));
        let focus = composer.focus_handle(cx);
        let subscription = cx.on_focus(&focus, window, move |shell, window, cx| {
            if shell.tab_shown != Some(Tab::Session(session)) {
                shell.focus_shown(window, cx);
            }
        });
        let open_project = cx.subscribe(&composer, |shell, _, OpenProject(project), cx| {
            let project = *project;
            (shell.sidebar).update(cx, |sidebar, cx| sidebar.new_session_in(project, cx));
        });
        OpenComposer {
            composer,
            conversation,
            _focus: subscription,
            _open_project: open_project,
        }
    }

    /// The composer of the session on screen, if one is.
    pub(super) fn shown_composer(&self) -> Option<Entity<Composer>> {
        let session = self.tab_shown?.session()?;
        Some(self.composers.get(&session)?.composer.clone())
    }

    /// The conversation on screen, for a scene to read.
    #[cfg(all(target_os = "macos", test))]
    pub fn conversation(&self, cx: &App) -> Option<Entity<Conversation>> {
        self.shown_conversation(cx)
    }

    /// The conversation of the session on screen, once it has seen work: a
    /// draft has only its composer.
    pub(super) fn shown_conversation(&self, cx: &App) -> Option<Entity<Conversation>> {
        let session = self.tab_shown?.session()?;
        let workspace = self.store.read(cx).workspace();
        (!workspace.session(session)?.is_empty())
            .then(|| {
                self.composers
                    .get(&session)
                    .map(|open| open.conversation.clone())
            })
            .flatten()
    }

    /// Puts the cursor in the composer on screen, else gives focus to the
    /// window, so typing never goes to a composer out of sight.
    pub(super) fn focus_shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = match self.shown_composer() {
            _ if self.settings_open => self.settings.focus_handle(cx),
            Some(composer) => composer.focus_handle(cx),
            None => self.focus_handle.clone(),
        };
        window.focus(&focus, cx);
    }

    /// Whether focus is the window's or a composer's, rather than a palette's,
    /// a menu's, the quit dialog's or the search's.
    fn focus_is_free(&self, window: &Window, cx: &Context<Self>) -> bool {
        let Some(focused) = window.focused(cx) else {
            return true;
        };
        focused == self.focus_handle
            || self.settings.focus_handle(cx).contains(&focused, window)
            || self
                .composers
                .values()
                .any(|open| open.composer.focus_handle(cx) == focused)
    }
}
