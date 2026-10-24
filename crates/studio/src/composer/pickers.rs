//! The session's choices, each a pill in the toolbar whose menu picks it: the
//! model (under its agent) and the effort. A pick is saved at once and
//! becomes what new sessions start from. Access is not a choice: every
//! session runs in Full access.

use gpui::{App, Context, Hsla, SharedString};
use theme::{Colors, Theme};
use workspace::{AGENTS, Agent, AgentChoice, AgentId, ModelId};

use super::Composer;
use crate::{
    agents::logo,
    assets::Icon,
    menu::{Entry, Item},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Picker {
    Model,
    Effort,
}

impl Picker {
    pub const ALL: [Picker; 2] = [Picker::Model, Picker::Effort];

    /// The pill's element id; its menu's is `menu_id`.
    pub fn id(self) -> &'static str {
        match self {
            Picker::Model => "composer-model",
            Picker::Effort => "composer-effort",
        }
    }

    pub fn menu_id(self) -> &'static str {
        match self {
            Picker::Model => "composer-model-menu",
            Picker::Effort => "composer-effort-menu",
        }
    }

    /// What the pill's tooltip names it.
    pub fn name(self) -> &'static str {
        match self {
            Picker::Model => "Model",
            Picker::Effort => "Effort",
        }
    }

    /// The pill's value: a model this build doesn't know shows as written.
    pub fn value(self, choice: &AgentChoice) -> SharedString {
        match self {
            Picker::Model => workspace::agent(&choice.agent)
                .and_then(|agent| agent.model(&choice.model))
                .map_or_else(|| choice.model.0.clone().into(), |model| model.label.into()),
            Picker::Effort => choice.effort.label().into(),
        }
    }

    /// The mark before the pill's value, in its own color: the agent's logo
    /// for the model, the gauge for the effort.
    pub fn mark(self, choice: &AgentChoice, colors: Colors) -> Option<(Icon, Hsla)> {
        match self {
            Picker::Model => logo(&choice.agent, colors),
            Picker::Effort => Some((Icon::Usage, colors.text_secondary)),
        }
    }
}

/// What the session's agent offers; one this build doesn't know offers what
/// the first agent does.
fn offers(choice: &AgentChoice) -> &'static Agent {
    workspace::agent(&choice.agent).unwrap_or(&AGENTS[0])
}

impl Composer {
    /// What the session runs on, or would start from if it were gone.
    pub(super) fn choice(&self, cx: &App) -> AgentChoice {
        let workspace = self.store.read(cx).workspace();
        workspace.session(self.session).map_or_else(
            || workspace.last_choice().clone(),
            |session| session.choice.clone(),
        )
    }

    /// Changes the session's choice, saves it, and tells its agent if one
    /// runs.
    fn choose(&mut self, change: impl FnOnce(&mut AgentChoice), cx: &mut Context<Self>) {
        let old = self.choice(cx);
        let mut choice = old.clone();
        change(&mut choice);
        self.agent
            .update(cx, |agent, _| agent.choice_changed(&old, &choice));
        let session = self.session;
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| workspace.set_choice(session, choice))
        });
    }

    pub(super) fn picker_entries(&self, picker: Picker, cx: &App) -> Vec<Entry<Self>> {
        let choice = self.choice(cx);
        match picker {
            Picker::Model => model_entries(&choice, Theme::of(cx).colors),
            Picker::Effort => (offers(&choice).efforts.iter().enumerate())
                .map(|(at, level)| {
                    let effort = level.effort;
                    Entry::Item(
                        Item::new(
                            ("effort", at as u64),
                            effort.label(),
                            move |composer: &mut Composer, _, cx| {
                                composer.choose(|choice| choice.effort = effort, cx)
                            },
                        )
                        .checked(choice.effort == effort),
                    )
                })
                .collect(),
        }
    }
}

/// Each agent's name over its models. A model or agent this build doesn't
/// know shows as written, checked, under its agent.
fn model_entries(choice: &AgentChoice, colors: Colors) -> Vec<Entry<Composer>> {
    let mut entries = Vec::new();
    let mut next_id = 0u64;
    let mut item = |label: SharedString, agent: &str, model: &str, checked: bool| {
        next_id += 1;
        let (agent, model) = (AgentId(agent.to_owned()), ModelId(model.to_owned()));
        let mark = logo(&agent, colors);
        let item = Item::new(
            ("model", next_id),
            label,
            move |composer: &mut Composer, _, cx| {
                let (agent, model) = (agent.clone(), model.clone());
                composer.choose(
                    |choice| {
                        choice.agent = agent;
                        choice.model = model;
                    },
                    cx,
                )
            },
        )
        .checked(checked);
        Entry::Item(match mark {
            Some((icon, color)) => item.icon(icon, color),
            None => item,
        })
    };
    let current = &choice.model.0;
    for agent in AGENTS {
        entries.push(Entry::Section(agent.name.into()));
        let chosen = choice.agent.0 == agent.id;
        for model in agent.models {
            let checked = chosen && *current == model.id;
            entries.push(item(model.label.into(), agent.id, model.id, checked));
        }
        if chosen && agent.model(&choice.model).is_none() {
            entries.push(item(current.clone().into(), agent.id, current, true));
        }
    }
    if workspace::agent(&choice.agent).is_none() {
        entries.push(Entry::Section(choice.agent.0.clone().into()));
        entries.push(item(current.clone().into(), &choice.agent.0, current, true));
    }
    entries
}
