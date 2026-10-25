//! The coding agents a session can run, what each offers, and a session's
//! choice among them. The catalog is a table: the next agent is one more entry.

use serde::{Deserialize, Deserializer, Serialize, de::IntoDeserializer};

/// An agent by the command that runs it, such as `claude`. Kept as written,
/// so one this build doesn't know survives loading and saving.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub String);

/// A model as its agent's command line names it, such as `opus`. Kept as
/// written, so a retired model, or one a newer build offers, stays chosen.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(pub String);

/// How hard the agent thinks before it answers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effort {
    Low,
    /// What a session starts on, and what the menu recommends.
    #[default]
    Medium,
    High,
    ExtraHigh,
    Max,
}

impl Effort {
    pub fn label(self) -> &'static str {
        match self {
            Effort::Low => "Low",
            Effort::Medium => "Medium",
            Effort::High => "High",
            Effort::ExtraHigh => "Extra high",
            Effort::Max => "Max",
        }
    }
}

/// What the agent may do without asking first. The app runs every session
/// in Full access; the others stay so a saved choice and the agent's
/// protocol still read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Access {
    AskFirst,
    AcceptEdits,
    Plan,
    Auto,
    #[default]
    FullAccess,
}

/// An agent the app can run, as its command line offers it.
#[derive(Debug)]
pub struct Agent {
    pub id: &'static str,
    pub name: &'static str,
    /// How to install it, as a command to run in a terminal.
    pub install: &'static str,
    /// In the order the model menu lists them.
    pub models: &'static [Model],
    pub default_model: &'static str,
    pub efforts: &'static [EffortLevel],
    pub access: &'static [AccessMode],
}

#[derive(Debug)]
pub struct Model {
    /// What follows `--model`.
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug)]
pub struct EffortLevel {
    pub effort: Effort,
    /// What follows `--effort`.
    pub cli: &'static str,
}

#[derive(Debug)]
pub struct AccessMode {
    pub access: Access,
    /// What follows `--permission-mode`.
    pub cli: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

/// Every agent the app can run; the first is the one new sessions start with.
pub const AGENTS: &[Agent] = &[Agent {
    id: "claude",
    name: "Claude Code",
    install: if cfg!(windows) {
        "npm install -g @anthropic-ai/claude-code"
    } else {
        "curl -fsSL https://claude.ai/install.sh | bash"
    },
    models: &[
        Model {
            id: "fable",
            label: "Fable 5.1",
        },
        Model {
            id: "opus",
            label: "Opus 5.5",
        },
        Model {
            id: "sonnet",
            label: "Sonnet 5.5",
        },
        Model {
            id: "haiku",
            label: "Haiku 5.5",
        },
    ],
    default_model: "opus",
    efforts: &[
        EffortLevel {
            effort: Effort::Low,
            cli: "low",
        },
        EffortLevel {
            effort: Effort::Medium,
            cli: "medium",
        },
        EffortLevel {
            effort: Effort::High,
            cli: "high",
        },
        EffortLevel {
            effort: Effort::ExtraHigh,
            cli: "xhigh",
        },
        EffortLevel {
            effort: Effort::Max,
            cli: "max",
        },
    ],
    access: &[
        AccessMode {
            access: Access::AskFirst,
            cli: "manual",
            label: "Ask first",
            description: "Asks before edits and commands",
        },
        AccessMode {
            access: Access::AcceptEdits,
            cli: "acceptEdits",
            label: "Accept edits",
            description: "Edits files, asks before commands",
        },
        AccessMode {
            access: Access::Plan,
            cli: "plan",
            label: "Plan",
            description: "Reads and plans, changes nothing",
        },
        AccessMode {
            access: Access::Auto,
            cli: "auto",
            label: "Auto",
            description: "Decides what needs asking",
        },
        AccessMode {
            access: Access::FullAccess,
            cli: "bypassPermissions",
            label: "Full access",
            description: "Never asks",
        },
    ],
}];

/// The agent with this id, if this build knows it.
pub fn agent(id: &AgentId) -> Option<&'static Agent> {
    AGENTS.iter().find(|agent| agent.id == id.0)
}

impl Agent {
    pub fn model(&self, id: &ModelId) -> Option<&'static Model> {
        let models: &'static [Model] = self.models;
        models.iter().find(|model| model.id == id.0)
    }

    pub fn access_mode(&self, access: Access) -> Option<&'static AccessMode> {
        let modes: &'static [AccessMode] = self.access;
        modes.iter().find(|mode| mode.access == access)
    }

    /// The command-line arguments that start this agent on `choice`. A model
    /// it doesn't list goes as written; an effort or access it lacks is left
    /// to the agent's own default.
    pub fn arguments<'a>(&self, choice: &'a AgentChoice) -> Vec<&'a str> {
        let mut arguments = vec!["--model", choice.model.0.as_str()];
        if let Some(level) = self
            .efforts
            .iter()
            .find(|level| level.effort == choice.effort)
        {
            arguments.extend(["--effort", level.cli]);
        }
        if let Some(mode) = self.access_mode(choice.access) {
            arguments.extend(["--permission-mode", mode.cli]);
        }
        arguments
    }
}

/// What a session runs: which agent and model, how hard it thinks, and what
/// it may do without asking. Missing from a saved session, a part reads as
/// the default; an effort or access a newer build wrote reads as the default
/// too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentChoice {
    pub agent: AgentId,
    pub model: ModelId,
    #[serde(deserialize_with = "known_or_default")]
    pub effort: Effort,
    #[serde(deserialize_with = "known_or_default")]
    pub access: Access,
}

impl Default for AgentChoice {
    /// The first agent on its default model, at High, in Full access.
    fn default() -> Self {
        let agent = &AGENTS[0];
        Self {
            agent: AgentId(agent.id.to_owned()),
            model: ModelId(agent.default_model.to_owned()),
            effort: Effort::default(),
            access: Access::default(),
        }
    }
}

/// Reads a name this build may not know as the default rather than failing
/// the whole file.
fn known_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let name = String::deserialize(deserializer)?;
    let name: serde::de::value::StringDeserializer<serde::de::value::Error> =
        name.into_deserializer();
    Ok(T::deserialize(name).unwrap_or_default())
}
