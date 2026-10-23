//! The models a session can pick from: those the agent offers, as its answer
//! to `initialize` lists them, with the catalog's list as the fallback.

use serde_json::Value;
use workspace::{AgentChoice, Effort, ModelId};

use crate::Transcript;

/// A model to pick, and the efforts it takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOption {
    pub id: ModelId,
    pub label: String,
    pub description: String,
    /// In the order the agent lists them, lowest first; empty for a model
    /// that takes no effort.
    pub efforts: Vec<Effort>,
}

/// A model as the agent offers it, its efforts as the agent names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Offered {
    id: String,
    label: String,
    description: String,
    efforts: Vec<String>,
}

/// The models in the agent's answer to `initialize`, but its "default", which
/// stands for one of the others, and any it marks hidden or disabled.
pub(crate) fn offered(response: &Value) -> Vec<Offered> {
    let rows = response.get("models").and_then(Value::as_array);
    let text = |row: &Value, key| {
        row.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let flag = |row: &Value, key| row.get(key).and_then(Value::as_bool).unwrap_or(false);
    rows.into_iter()
        .flatten()
        .filter(|row| !flag(row, "hidden") && !flag(row, "disabled"))
        .filter_map(|row| {
            let id = text(row, "value");
            let skipped = id.is_empty() || id == "default" || id.starts_with("cc-update-required");
            let efforts = row.get("supportedEffortLevels").and_then(Value::as_array);
            let efforts = efforts.into_iter().flatten().filter_map(Value::as_str);
            (!skipped).then(|| Offered {
                label: Some(text(row, "displayName"))
                    .filter(|label| !label.is_empty())
                    .unwrap_or_else(|| id.clone()),
                description: text(row, "description"),
                efforts: efforts.map(str::to_owned).collect(),
                id,
            })
        })
        .collect()
}

impl Transcript {
    /// The models to offer for a session on `choice`: the agent's own once it
    /// has answered, else the catalog's. The chosen model is always among
    /// them, at the end when neither lists it, so one the agent no longer
    /// names, or a newer build added, stays usable.
    pub fn models(&self, choice: &AgentChoice) -> Vec<ModelOption> {
        let agent = workspace::agent(&choice.agent);
        let every_effort = || {
            agent
                .map(|agent| agent.efforts.iter().map(|level| level.effort).collect())
                .unwrap_or_default()
        };
        let mut models: Vec<ModelOption> = if self.offered().is_empty() {
            let catalog = agent.map(|agent| agent.models).unwrap_or_default();
            catalog
                .iter()
                .map(|model| ModelOption {
                    id: ModelId(model.id.to_owned()),
                    label: model.label.to_owned(),
                    description: String::new(),
                    efforts: every_effort(),
                })
                .collect()
        } else {
            let effort = |name: &String| {
                let levels = agent.map(|agent| agent.efforts).unwrap_or_default();
                levels
                    .iter()
                    .find(|level| level.cli == name)
                    .map(|level| level.effort)
            };
            let offered = self.offered().iter();
            offered
                .map(|model| ModelOption {
                    id: ModelId(model.id.clone()),
                    label: model.label.clone(),
                    description: model.description.clone(),
                    efforts: model.efforts.iter().filter_map(effort).collect(),
                })
                .collect()
        };
        if !models.iter().any(|model| model.id == choice.model) {
            models.push(ModelOption {
                id: choice.model.clone(),
                label: choice.model.0.clone(),
                description: String::new(),
                efforts: every_effort(),
            });
        }
        models
    }
}
