//! The dispatcher's fixtures: a call, a registry, a root, and a scripted interactor.

use serde_json::Value;

use perch_agent::ToolCall;
use perch_agent::dispatch::{Answer, Decision, Dispatcher, Interactor};
use perch_tools::{Preview, ToolCtx, ToolRegistry};

pub fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args,
    }
}

pub fn registry() -> ToolRegistry {
    Dispatcher::builtin_registry()
}

pub fn ctx(root: &std::path::Path) -> ToolCtx {
    ToolCtx {
        root: root.to_path_buf(),
    }
}

/// An interactor whose every answer is scripted, so a test states the user's behaviour and
/// nothing else.
pub struct Script {
    /// How many times the dispatcher asked for approval.
    pub prompts: usize,
    /// What each approval gets: consumed in order, the last one repeating.
    decisions: Vec<Decision>,
    /// The preview each approval was shown.
    pub previews: Vec<Option<Preview>>,
    answer: Answer,
}

impl Script {
    pub fn deny_all() -> Self {
        Self {
            prompts: 0,
            decisions: vec![Decision::Deny],
            previews: Vec::new(),
            answer: Answer::Cancelled,
        }
    }

    /// Allow the first prompt for the session, refuse every later one, so a second prompt is
    /// visible both as a count and as a call that did not run.
    pub fn allow_session_once() -> Self {
        Self {
            prompts: 0,
            decisions: vec![Decision::AllowForSession, Decision::Deny],
            previews: Vec::new(),
            answer: Answer::Cancelled,
        }
    }

    pub fn answer(answer: Answer) -> Self {
        Self {
            prompts: 0,
            decisions: vec![Decision::Allow],
            previews: Vec::new(),
            answer,
        }
    }
}

impl Interactor for Script {
    fn approve(&mut self, _name: &str, preview: Option<&Preview>) -> Decision {
        self.prompts += 1;
        self.previews.push(preview.cloned());
        if self.decisions.len() > 1 {
            self.decisions.remove(0)
        } else {
            self.decisions[0]
        }
    }

    fn ask(&mut self, _prompt: &str, _options: &[String], _multiple: bool) -> Answer {
        self.answer.clone()
    }
}
