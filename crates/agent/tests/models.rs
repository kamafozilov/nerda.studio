//! The models a session offers: the agent's own list, the catalog's before
//! the agent answers, and the chosen one always.

mod support;

use agent::{Event, Incoming, ModelOption, RequestId, Transcript};
use serde_json::json;
use support::{Player, says};
use workspace::{AgentChoice, Effort, ModelId};

const EVERY: [Effort; 5] = [
    Effort::Low,
    Effort::Medium,
    Effort::High,
    Effort::ExtraHigh,
    Effort::Max,
];

fn choice(model: &str) -> AgentChoice {
    AgentChoice {
        model: ModelId(model.to_owned()),
        ..AgentChoice::default()
    }
}

fn ids(models: &[ModelOption]) -> Vec<&str> {
    models.iter().map(|model| model.id.0.as_str()).collect()
}

#[test]
fn the_agent_s_own_models_and_their_efforts_once_it_answers() {
    let mut player = Player::new();
    player.play("initialize.jsonl", says("unused"));
    let models = player.transcript.models(&choice("opus"));
    assert_eq!(
        models,
        [ModelOption {
            id: ModelId("opus".to_owned()),
            label: "Opus 5.5".to_owned(),
            description: "For complex work and everyday tasks".to_owned(),
            efforts: EVERY.to_vec(),
        }],
        "the agent's default stands for one of the others"
    );
}

#[test]
fn before_the_agent_answers_the_catalog_s_models_are_offered() {
    let transcript = Transcript::new();
    let models = transcript.models(&choice("opus"));
    assert_eq!(ids(&models), ["fable", "opus", "sonnet", "haiku"]);
    assert!(models.iter().all(|model| model.efforts == EVERY));
}

#[test]
fn a_model_neither_lists_stays_offered_and_efforts_the_build_lacks_are_left_out() {
    let mut transcript = Transcript::new();
    let answer = json!({"models": [
        {"value": "default", "displayName": "Default (recommended)", "supportedEffortLevels": ["low"]},
        {"value": "sonnet", "displayName": "Sonnet 5.5", "description": "Fast", "supportedEffortLevels": ["low", "high", "ultra"]},
        {"value": "haiku", "displayName": "Haiku 5.5", "supportsEffort": false},
        {"value": "secret", "displayName": "Secret", "hidden": true},
        {"value": "cc-update-required-opus", "displayName": "Update to use"},
    ]});
    transcript.apply(&Event::Received(Incoming::Response {
        id: RequestId("initialize".to_owned()),
        result: Ok(answer),
    }));
    let models = transcript.models(&choice("claude-opus-4-9"));
    assert_eq!(ids(&models), ["sonnet", "haiku", "claude-opus-4-9"]);
    assert_eq!(models[0].efforts, [Effort::Low, Effort::High]);
    assert!(models[1].efforts.is_empty(), "haiku takes no effort here");
    assert_eq!(models[2].label, "claude-opus-4-9");
    assert_eq!(models[2].efforts, EVERY);

    let unknown_agent = AgentChoice {
        agent: workspace::AgentId("someday".to_owned()),
        ..choice("x")
    };
    assert_eq!(ids(&Transcript::new().models(&unknown_agent)), ["x"]);
}
