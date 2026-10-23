//! One composer per open session, so a message half written survives
//! switching tabs. Showing a session's tab puts the cursor in its composer,
//! unless a palette, a menu, the quit dialog or the search holds it.

use gpui::{AppContext, Context, Entity, Focusable, Subscription, Window};
use workspace::{SessionId, Tab};

use super::Shell;
use crate::composer::Composer;

pub(super) struct OpenComposer {
    pub composer: Entity<Composer>,
    /// Sends focus on to the composer on screen, should it come back here
    /// while this session is off screen, as when a palette closes.
    _focus: Subscription,
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
        let composer = cx.new(|cx| Composer::new(store, session, cx));
        let focus = composer.focus_handle(cx);
        let subscription = cx.on_focus(&focus, window, move |shell, window, cx| {
            if shell.tab_shown != Some(Tab::Session(session)) {
                shell.focus_shown(window, cx);
            }
        });
        OpenComposer {
            composer,
            _focus: subscription,
        }
    }

    /// The composer of the session on screen, if one is.
    pub(super) fn shown_composer(&self) -> Option<Entity<Composer>> {
        let session = self.tab_shown?.session()?;
        Some(self.composers.get(&session)?.composer.clone())
    }

    /// Puts the cursor in the composer on screen, else gives focus to the
    /// window, so typing never goes to a composer out of sight.
    pub(super) fn focus_shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = match self.shown_composer() {
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
            || self
                .composers
                .values()
                .any(|open| open.composer.focus_handle(cx) == focused)
    }
}
