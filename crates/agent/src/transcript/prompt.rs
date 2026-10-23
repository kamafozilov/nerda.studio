//! The agent's prompts, waiting for the person, and the person's answers.

use crate::protocol::{self, Ask, Command, Decision, PermissionRequest};

#[derive(Clone, Debug, PartialEq)]
pub struct Permission {
    pub request: PermissionRequest,
    pub state: PermissionState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionState {
    Pending,
    Allowed,
    Denied,
    /// Withdrawn by the agent, or left unanswered when the turn ended.
    Cancelled,
}

/// The person's answer to a prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Run the tool as asked; for a plan, start on it.
    AllowOnce,
    /// Run it, and let it run without asking for the rest of the session:
    /// the agent's suggested rules, kept for the session only. The same as
    /// `AllowOnce` where the agent offers no such rule, and for questions
    /// and plans.
    AllowForSession,
    /// Don't run it, with what the person said, which the agent reads; for a
    /// plan, keep planning.
    Deny { feedback: Option<String> },
    /// Don't run it, and stop the turn.
    Cancel,
}

impl Permission {
    /// The command that gives the agent `answer`.
    pub fn answer(&self, answer: Answer) -> Command {
        let request = &self.request;
        let decision = match answer {
            Answer::AllowOnce => allow(request.input.clone(), Vec::new()),
            Answer::AllowForSession if request.ask == Ask::Tool && request.session_rule => {
                let permissions = protocol::for_session(&request.suggestions, &request.tool_name);
                allow(request.input.clone(), permissions)
            }
            Answer::AllowForSession => allow(request.input.clone(), Vec::new()),
            Answer::Deny { feedback } => Decision::Deny {
                message: denied(&request.ask, feedback.as_deref()),
                interrupt: false,
            },
            Answer::Cancel => Decision::Deny {
                message: "The user stopped the turn.".to_owned(),
                interrupt: true,
            },
        };
        self.decide(decision)
    }

    /// The command that answers the agent's questions: `answers[i]` holds the
    /// labels picked for the i-th, or the person's own words. For any other
    /// prompt, the same as `Answer::AllowOnce`.
    pub fn answer_questions(&self, answers: &[Vec<String>]) -> Command {
        let request = &self.request;
        let input = match &request.ask {
            Ask::Questions(questions) => protocol::answered(&request.input, questions, answers),
            _ => request.input.clone(),
        };
        self.decide(allow(input, Vec::new()))
    }

    fn decide(&self, decision: Decision) -> Command {
        Command::Answer {
            request: self.request.id.clone(),
            decision,
        }
    }
}

fn allow(input: serde_json::Value, permissions: Vec<serde_json::Value>) -> Decision {
    Decision::Allow { input, permissions }
}

/// What the agent reads when the person says no.
fn denied(ask: &Ask, feedback: Option<&str>) -> String {
    let what = match ask {
        Ask::Tool => "The user denied this tool call",
        Ask::Questions(_) => "The user didn't answer the questions",
        Ask::Plan(_) => "The user wants to keep planning, not start on this plan yet",
    };
    match feedback
        .map(str::trim)
        .filter(|feedback| !feedback.is_empty())
    {
        Some(feedback) => format!("{what}, and said: {feedback}"),
        None => format!("{what}."),
    }
}
