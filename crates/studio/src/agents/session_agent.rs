//! One session's agent as the app holds it: the transcript the screen draws,
//! the process when one runs, and the session's journal. The process starts
//! on the first message after the app opened, resuming the conversation when
//! it has a turn on record, and again after one ended on its own.

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use agent::{
    Attachment, Command, Conversation, Event, Events, Incoming, Item, Launch, Message, Status,
    Transcript, TurnState,
};
use gpui::{App, AppContext, Context, Entity, EventEmitter, SharedString, Task};
use workspace::{Access, AgentChoice, SessionId, Timestamp};

use super::journal::{self, Journal, Journals, Lost, Replayed};
use crate::store::Store;

/// Something about the agent to show in the composer's problem line.
pub struct Problem(pub SharedString);

pub struct SessionAgent {
    session: SessionId,
    store: Entity<Store>,
    /// Runs in place of the agent's own command.
    program: Option<PathBuf>,
    journals: Option<Journals>,
    journal: Option<Journal>,
    transcript: Transcript,
    state: State,
    /// The conversation has a turn on record, so the next process resumes it.
    recorded: bool,
    /// Full access is being switched, which ends the process and starts
    /// another: until it has, an exit isn't the agent going away.
    switching: bool,
    /// The access the process runs in, which a refused switch puts back.
    access: Option<Access>,
    /// Since when a turn has been running or waiting, while one is.
    busy_since: Option<Instant>,
    /// When each thinking block still streaming began, by turn and item.
    thinking_since: HashMap<(usize, usize), Instant>,
}

enum State {
    /// The journal is being read; nothing is sent until it has been.
    Loading { _replay: Task<()> },
    /// No process has run since the app opened.
    Idle,
    /// The process, while it runs and after it ended; its events are
    /// drained into the transcript and the journal as they come.
    Started {
        agent: agent::Session,
        events: Events,
        _drain: Task<()>,
    },
}

impl EventEmitter<Problem> for SessionAgent {}

impl SessionAgent {
    /// The agent of `session`, its journal read first when it has one.
    pub fn open(
        session: SessionId,
        store: Entity<Store>,
        program: Option<PathBuf>,
        journals: Option<Journals>,
        cx: &mut Context<Self>,
    ) -> Self {
        let conversation = (store.read(cx).workspace().session(session))
            .and_then(|session| session.conversation.clone());
        let journal = journals
            .as_ref()
            .zip(conversation)
            .map(|(journals, id)| journals.of(&id));
        let state = match &journal {
            Some(journal) => {
                let path = journal.path().to_owned();
                State::Loading {
                    _replay: cx.spawn(async move |this, cx| {
                        let read = cx.background_spawn(async move { journal::read(&path) });
                        let replayed = read.await;
                        this.update(cx, |this, cx| this.replayed(replayed, cx)).ok();
                    }),
                }
            }
            None => State::Idle,
        };
        Self {
            session,
            store,
            program,
            journals,
            journal,
            transcript: Transcript::new(),
            state,
            recorded: false,
            switching: false,
            access: None,
            busy_since: None,
            thinking_since: HashMap::new(),
        }
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub fn busy_since(&self) -> Option<Instant> {
        self.busy_since
    }

    /// When the thinking at `item` of `turn` began, while it streams.
    pub fn thinking_since(&self, turn: usize, item: usize) -> Option<Instant> {
        self.thinking_since.get(&(turn, item)).copied()
    }

    /// Whether a turn runs or a message waits for one.
    pub fn is_busy(&self) -> bool {
        self.busy_since.is_some()
    }

    /// Sends a message of `text` and the files at `paths`, starting the agent
    /// first when none runs. Fails, sending nothing, when it can't start.
    pub fn send(
        &mut self,
        text: String,
        paths: &[PathBuf],
        cx: &mut Context<Self>,
    ) -> Result<(), SharedString> {
        if matches!(self.state, State::Loading { .. }) {
            return Err("This conversation is still loading. Send it again in a moment.".into());
        }
        if !self.runs() {
            self.start(cx)?;
        }
        let State::Started { agent, .. } = &self.state else {
            return Ok(());
        };
        let attachments = paths.iter().map(|path| Attachment::open(path)).collect();
        agent.send(Command::Send(Message::new(text.as_str(), attachments)));
        let session = self.session;
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                workspace.record_message(session, &text, Timestamp::now())
            })
        });
        Ok(())
    }

    /// Stops the turn that runs and the messages that wait.
    pub fn stop(&mut self) {
        if let (true, State::Started { agent, .. }) = (self.runs(), &self.state) {
            agent.send(Command::Interrupt);
        }
    }

    /// Tells a running agent what changed between `old` and `new`; the next
    /// process starts on `new` anyway.
    pub fn choice_changed(&mut self, old: &AgentChoice, new: &AgentChoice) {
        let (true, State::Started { agent, .. }) = (self.runs(), &self.state) else {
            return;
        };
        if old.model != new.model {
            agent.send(Command::SetModel(new.model.clone()));
        }
        if old.effort != new.effort {
            agent.send(Command::SetEffort(new.effort));
        }
        if old.access != new.access {
            let full = |access: Access| access == Access::FullAccess;
            self.switching |= full(old.access) != full(new.access);
            agent.send(Command::SetAccess(new.access));
        }
    }

    /// Hands over what quitting waits on: the events still to come and the
    /// journal they go to. Dropping the process asks it to end.
    pub fn end(&mut self) -> Option<(Events, Option<Journal>)> {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Started { agent, events, .. } => {
                drop(agent);
                Some((events, self.journal.clone()))
            }
            _ => None,
        }
    }

    /// Whether a process runs that takes messages.
    fn runs(&self) -> bool {
        let gone = matches!(
            self.transcript.status(),
            Status::Exited { .. } | Status::Failed { .. }
        );
        matches!(self.state, State::Started { .. }) && (self.switching || !gone)
    }

    fn start(&mut self, cx: &mut Context<Self>) -> Result<(), SharedString> {
        let workspace = self.store.read(cx).workspace();
        let session = workspace
            .session(self.session)
            .ok_or("This session is gone.")?;
        let folder = workspace
            .project(session.project)
            .map(|project| project.path.clone())
            .ok_or("This session's project is gone.")?;
        let saved = session.conversation.clone();
        let id = saved.clone().unwrap_or_default();
        let launch = Launch {
            folder,
            choice: session.choice.clone(),
            conversation: match self.recorded {
                true => Conversation::Resume(id.clone()),
                false => Conversation::New(id.clone()),
            },
            program: self.program.clone(),
        };
        let (agent, events) = agent::Session::start(&launch)
            .map_err(|error| SharedString::from(error.to_string()))?;
        if saved.is_none() {
            let session = self.session;
            let conversation = id.clone();
            self.store.update(cx, |store, cx| {
                store.update(cx, |workspace| {
                    workspace.set_conversation(session, conversation)
                })
            });
        }
        if self.journal.is_none() {
            self.journal = self.journals.as_ref().map(|journals| journals.of(&id));
        }
        self.access = Some(launch.choice.access);
        self.switching = false;
        let drain = cx.spawn({
            let events = events.clone();
            async move |this, cx| {
                while let Ok(first) = events.recv().await {
                    let mut batch = vec![first];
                    batch.extend(std::iter::from_fn(|| events.try_recv().ok()));
                    if this.update(cx, |this, cx| this.apply(batch, cx)).is_err() {
                        break;
                    }
                }
            }
        });
        self.state = State::Started {
            agent,
            events,
            _drain: drain,
        };
        Ok(())
    }

    fn apply(&mut self, batch: Vec<Event>, cx: &mut Context<Self>) {
        let mut problem = None;
        for event in batch {
            self.transcript.apply(&event);
            match &event {
                Event::Received(Incoming::Init(_)) => self.recorded = true,
                Event::Started | Event::Failed(_) => self.switching = false,
                Event::Sent(Command::SetAccess(access)) => self.access = Some(*access),
                Event::Refused { command, reason } => {
                    self.switching = false;
                    problem = Some(SharedString::from(reason.to_string()));
                    if let (Command::SetAccess(_), Some(access)) = (command, self.access) {
                        self.put_back(access, cx);
                    }
                }
                _ => {}
            }
            if let Some(journal) = &self.journal {
                journal.append(event);
            }
        }
        self.note_times();
        if self.journals.as_ref().is_some_and(Journals::take_failure) {
            self.journal_failed(cx);
        }
        if let Some(problem) = problem {
            cx.emit(Problem(problem));
        }
        cx.notify();
    }

    fn replayed(&mut self, replayed: Option<Replayed>, cx: &mut Context<Self>) {
        let Some(replayed) = replayed else {
            // A conversation without a journal ran before journals were
            // kept, or lost its file: it most likely has a turn to resume.
            self.recorded = true;
            self.state = State::Idle;
            return cx.notify();
        };
        self.transcript = Transcript::replay(&replayed.events);
        self.recorded = (replayed.events.iter())
            .any(|event| matches!(event, Event::Received(Incoming::Init(_))));
        // A journal cut short by a crash holds no exit: the process went
        // with the app, so what it had open ended there.
        let gone = matches!(
            self.transcript.status(),
            Status::Exited { .. } | Status::Failed { .. }
        );
        if !gone && !replayed.events.is_empty() {
            self.transcript.apply(&Event::Exited { code: None });
        }
        self.state = State::Idle;
        self.note_times();
        if let Some(notice) = self.lost_notice(replayed.lost, cx) {
            self.store
                .update(cx, |store, cx| store.show_notice(notice.into(), cx));
        }
        cx.notify();
    }

    /// What the sidebar says when part of the journal didn't read.
    fn lost_notice(&self, lost: Lost, cx: &App) -> Option<String> {
        let workspace = self.store.read(cx).workspace();
        let title = workspace.session(self.session)?.display_title();
        Some(match lost {
            Lost::Nothing => return None,
            Lost::Lines(1) => format!(
                "Part of \u{201c}{title}\u{201d} couldn\u{2019}t be read: one event of its conversation is left out."
            ),
            Lost::Lines(lines) => format!(
                "Part of \u{201c}{title}\u{201d} couldn\u{2019}t be read: {lines} events of its conversation are left out."
            ),
            Lost::All => format!(
                "\u{201c}{title}\u{201d} couldn\u{2019}t be read, so its conversation doesn\u{2019}t show."
            ),
        })
    }

    /// Starts or ends the clocks the screen shows: how long the agent has
    /// been working, and how long each thinking block has streamed.
    fn note_times(&mut self) {
        let now = Instant::now();
        let turns = self.transcript.turns();
        let busy = (turns.iter()).any(|turn| !matches!(turn.state, TurnState::Ended(_)));
        self.busy_since = busy.then(|| self.busy_since.unwrap_or(now));
        let mut thinking = HashMap::new();
        for (at, turn) in turns.iter().enumerate() {
            for (item, part) in turn.items.iter().enumerate() {
                if let Item::Thinking(text) = part
                    && text.streaming
                {
                    let since = self.thinking_since.get(&(at, item)).copied();
                    thinking.insert((at, item), since.unwrap_or(now));
                }
            }
        }
        self.thinking_since = thinking;
    }

    /// A refused switch of access: the session's choice goes back to what
    /// the agent runs in, so the pill tells the truth.
    fn put_back(&mut self, access: Access, cx: &mut Context<Self>) {
        let session = self.session;
        self.store.update(cx, |store, cx| {
            let choice = store
                .workspace()
                .session(session)
                .map(|session| AgentChoice {
                    access,
                    ..session.choice.clone()
                });
            if let Some(choice) = choice {
                store.update(cx, |workspace| workspace.set_choice(session, choice));
            }
        });
    }

    fn journal_failed(&mut self, cx: &mut Context<Self>) {
        let Some(journals) = &self.journals else {
            return;
        };
        let folder = workspace::home_relative(journals.folder());
        let notice = format!(
            "Couldn\u{2019}t save a conversation in {folder}, so it won\u{2019}t show after a restart."
        );
        self.store
            .update(cx, |store, cx| store.show_notice(notice.into(), cx));
    }
}

/// Waits for each ended agent's last events, up to `within` for them all,
/// writing each to its journal.
pub fn wait_for_ends(ending: Vec<(Events, Option<Journal>)>, within: Duration) {
    let until = Instant::now() + within;
    for (events, journal) in ending {
        loop {
            match events.try_recv() {
                Ok(event) => {
                    if let Some(journal) = &journal {
                        journal.append(event);
                    }
                }
                Err(error) if error.is_empty() && Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    }
}
