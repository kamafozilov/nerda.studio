//! What a permission prompt asks, by the tool asking: a tool wanting to run,
//! the agent's questions for the person, or its plan to approve.

use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// A tool wants to run on its input.
    Tool,
    /// The agent asks the person these, before it goes on. Answer them with
    /// `Permission::answer_questions`.
    Questions(Vec<Question>),
    /// The agent proposes this plan, in Markdown, and asks to start on it.
    /// `Answer::AllowOnce` approves it; `Answer::Deny` keeps planning, its
    /// feedback telling the agent what to change.
    Plan(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    /// The question in full; the answer goes back under it.
    pub question: String,
    /// A short label for it, such as `Format`.
    pub header: String,
    pub options: Vec<Choice>,
    /// The person may pick more than one option.
    pub multi_select: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub label: String,
    pub description: String,
}

impl Ask {
    /// What `tool` asks with `input`; a question or plan whose input doesn't
    /// read as one asks as any other tool does.
    pub(crate) fn of(tool: &str, input: &Value) -> Self {
        match tool {
            "AskUserQuestion" => {
                let questions = questions(input);
                if questions.is_empty() {
                    Ask::Tool
                } else {
                    Ask::Questions(questions)
                }
            }
            "ExitPlanMode" => match input.get("plan").and_then(Value::as_str) {
                Some(plan) => Ask::Plan(plan.to_owned()),
                None => Ask::Tool,
            },
            _ => Ask::Tool,
        }
    }
}

fn questions(input: &Value) -> Vec<Question> {
    let rows = input.get("questions").and_then(Value::as_array);
    let text = |row: &Value, keys: [&str; 2]| {
        let found = keys
            .iter()
            .find_map(|key| row.get(*key).and_then(Value::as_str));
        found.unwrap_or_default().to_owned()
    };
    rows.into_iter()
        .flatten()
        .filter_map(|row| {
            let question = text(row, ["question", "prompt"]);
            if question.trim().is_empty() {
                return None;
            }
            let options = row.get("options").and_then(Value::as_array);
            let options = options.into_iter().flatten().filter_map(|option| {
                let label = match option {
                    Value::String(label) => label.clone(),
                    option => text(option, ["label", "value"]),
                };
                let description = text(option, ["description", "detail"]);
                (!label.is_empty()).then_some(Choice { label, description })
            });
            Some(Question {
                header: text(row, ["header", "title"]),
                options: options.collect(),
                multi_select: row
                    .get("multiSelect")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                question,
            })
        })
        .collect()
}

/// The agent's input with the person's `answers` added under `answers`, each
/// keyed by its question's text: the label picked, or the labels when the
/// question takes several. `answers[i]` answers `questions[i]`; a question
/// without one is answered empty.
pub(crate) fn answered(input: &Value, questions: &[Question], answers: &[Vec<String>]) -> Value {
    let mut keyed = Map::new();
    for (at, question) in questions.iter().enumerate() {
        let picked = answers.get(at).cloned().unwrap_or_default();
        let value = match question.multi_select {
            true => Value::from(picked),
            false => Value::from(picked.into_iter().next().unwrap_or_default()),
        };
        keyed.insert(question.question.clone(), value);
    }
    let mut input = input.as_object().cloned().unwrap_or_default();
    input.insert("answers".to_owned(), Value::Object(keyed));
    Value::Object(input)
}

/// The agent's suggested permissions as rules for this session alone: rules
/// and folders it may use, never a change of mode, and never written to a
/// settings file, such as the project's `.claude/settings.local.json`. With
/// none to keep, a rule allowing `tool` for the session.
pub(crate) fn for_session(suggestions: &[Value], tool: &str) -> Vec<Value> {
    let mut kept: Vec<Value> = suggestions
        .iter()
        .filter(|suggestion| {
            let kind = suggestion.get("type").and_then(Value::as_str);
            matches!(kind, Some("addRules" | "addDirectories"))
        })
        .cloned()
        .map(|mut suggestion| {
            suggestion["destination"] = Value::from("session");
            suggestion
        })
        .collect();
    if kept.is_empty() {
        kept.push(serde_json::json!({
            "type": "addRules",
            "rules": [{"toolName": tool}],
            "behavior": "allow",
            "destination": "session",
        }));
    }
    kept
}
