//! Each session's agent, kept for as long as the app runs: started by the
//! session's first message, or by a message after the app restarted,
//! resuming its conversation. Closing a session's tab leaves its agent
//! running; quitting ends every agent and waits, a few seconds at most, for
//! each to exit, so nothing it started outlives the app.

mod journal;
mod session_agent;

use std::{collections::HashMap, path::PathBuf, time::Duration};

use gpui::{App, AppContext, Context, Entity, Global, Hsla, Subscription};
use theme::Colors;
use workspace::{AgentId, SessionId};

use journal::Journals;
pub use session_agent::{Problem, SessionAgent};

use crate::{assets::Icon, store::Store};

/// The agent's logo in its brand's color; an agent with none shows no mark.
pub fn logo(agent: &AgentId, colors: Colors) -> Option<(Icon, Hsla)> {
    match agent.0.as_str() {
        "claude" => Some((Icon::Claude, colors.claude)),
        _ => None,
    }
}

/// Names a program to run in place of each agent's own command, such as the
/// agent crate's `fake-agent`, so a build tried out plays a script instead
/// of calling a model.
const PROGRAM: &str = "NERDA_AGENT_PROGRAM";

/// How long quitting waits for the agents to end: an agent mid-turn gets
/// SIGTERM 2 s after its input closes, and SIGKILL 2 s after that.
const QUIT_WAIT: Duration = Duration::from_secs(6);

pub struct Agents {
    store: Entity<Store>,
    program: Option<PathBuf>,
    /// `None` while the workspace isn't saved: its sessions' conversations
    /// aren't either.
    journals: Option<Journals>,
    sessions: HashMap<SessionId, Entity<SessionAgent>>,
    _quit: Subscription,
}

/// The app's one registry of agents.
struct Registry(Entity<Agents>);

impl Global for Registry {}

/// Starts looking for the agents' program and keeps the registry for the app.
pub fn init(store: Entity<Store>, cx: &mut App) {
    agent::prewarm();
    let agents = cx.new(|cx| Agents::new(store, cx));
    cx.set_global(Registry(agents));
}

/// The agent of `session`, its journal replayed the first time it is asked for.
pub fn of(session: SessionId, cx: &mut App) -> Entity<SessionAgent> {
    let agents = cx.global::<Registry>().0.clone();
    agents.update(cx, |agents, cx| agents.open(session, cx))
}

impl Agents {
    fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let program = std::env::var_os(PROGRAM)
            .filter(|program| !program.is_empty())
            .map(PathBuf::from);
        #[cfg(test)]
        let program = program.or_else(crate::scene::agent_program);
        let journals = store.read(cx).data_dir().map(Journals::start);
        Self {
            store,
            program,
            journals,
            sessions: HashMap::new(),
            _quit: cx.on_app_quit(|agents, cx| {
                agents.end_all(cx);
                async {}
            }),
        }
    }

    fn open(&mut self, session: SessionId, cx: &mut Context<Self>) -> Entity<SessionAgent> {
        if let Some(agent) = self.sessions.get(&session) {
            return agent.clone();
        }
        let (store, program, journals) = (
            self.store.clone(),
            self.program.clone(),
            self.journals.clone(),
        );
        let agent = cx.new(|cx| SessionAgent::open(session, store, program, journals, cx));
        self.sessions.insert(session, agent.clone());
        agent
    }

    /// Ends every agent and waits for their exits to reach their journals.
    fn end_all(&mut self, cx: &mut Context<Self>) {
        let ending: Vec<_> = (self.sessions.values())
            .filter_map(|agent| agent.update(cx, |agent, _| agent.end()))
            .collect();
        session_agent::wait_for_ends(ending, QUIT_WAIT);
        if let Some(journals) = &self.journals {
            journals.flush(Duration::from_secs(1));
        }
    }
}
