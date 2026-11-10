//! Tool dispatch: the approval gate, execution, and one tool message per call.
//!
//! The dispatcher is the only place a tool the model asked for becomes a result. It consults the
//! registry for whether a call needs approval, asks the [`Interactor`], runs the call, and answers
//! every call with a [`Message::tool_result`] that repeats the call's id — the provider addresses
//! each result to the assistant's call by that id, so a lost or renumbered id is a rejected
//! conversation.
//!
//! Nothing here can panic into the turn: a tool that fails, an unknown tool, and a denied call all
//! become message content, because the model can act on a stated failure and cannot act on a
//! dead thread.

use std::collections::BTreeSet;
use std::sync::Arc;

use perch_tools::{Preview, ToolCtx, ToolRegistry};

use crate::ask::{self, AskUser};
use crate::event::ToolCall;
use crate::provider::Message;

/// What the user decided about one call that asked first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Run this call.
    Allow,
    /// Run this call, and stop asking about this tool for the rest of the session.
    AllowForSession,
    /// Do not run this call, and tell the model so.
    Deny,
}

/// What the user did with a question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Picked the option at this index.
    Pick(usize),
    /// Typed an answer of their own.
    Text(String),
    /// Dismissed the question without answering it.
    Cancelled,
}

/// Everything a turn needs from the person in front of the window.
///
/// One trait rather than callbacks so a headless test and the UI take the same path: the
/// dispatcher neither knows nor cares whether an approval came from a dialog or a script.
pub trait Interactor {
    /// Whether to run this call, with whatever the approval view can show about it.
    fn approve(&mut self, name: &str, preview: Option<&Preview>) -> Decision;

    /// The answer to one question. `options` is empty for a free-text question.
    fn ask(&mut self, prompt: &str, options: &[String], multiple: bool) -> Answer;
}

/// Runs the tool calls of one turn.
///
/// The "for session" allowance lives here, keyed by tool name, so it lasts as long as the
/// dispatcher does and covers only the tool the user allowed.
#[derive(Default)]
pub struct Dispatcher {
    allowed: BTreeSet<String>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// The six built-ins plus `ask_user`, which the model calls like any other tool.
    pub fn builtin_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::builtin();
        registry.register(Arc::new(AskUser));
        registry
    }

    /// Runs every call in order, answering each with one tool message that keeps the call's id.
    pub fn run(
        &mut self,
        calls: &[ToolCall],
        registry: &ToolRegistry,
        ctx: &ToolCtx,
        interactor: &mut dyn Interactor,
    ) -> Vec<Message> {
        calls
            .iter()
            .map(|call| {
                let text = self.one(call, registry, ctx, interactor);
                Message::tool_result(call.id.clone(), text)
            })
            .collect()
    }

    /// The text one call answers with, for good or ill.
    fn one(
        &mut self,
        call: &ToolCall,
        registry: &ToolRegistry,
        ctx: &ToolCtx,
        interactor: &mut dyn Interactor,
    ) -> String {
        match registry.needs_approval(&call.name) {
            Ok(false) => {}
            Ok(true) => {
                // A tool already allowed for the session is not asked about again; the allowance
                // is keyed by name, so allowing `bash` says nothing about `write`.
                if !self.allowed.contains(&call.name) {
                    let preview = preview_for(&call.name, &call.args);
                    match interactor.approve(&call.name, preview.as_ref()) {
                        Decision::Allow => {}
                        Decision::AllowForSession => {
                            self.allowed.insert(call.name.clone());
                        }
                        Decision::Deny => {
                            return format!(
                                "the user denied the {} call, so it did not run",
                                call.name
                            );
                        }
                    }
                }
            }
            // An unknown tool is refused before anything runs, which also keeps the question
            // bridge below from asking a user about a call that cannot be made.
            Err(error) => return error.to_string(),
        }

        if call.name == ask::ASK_USER {
            ask::answer_through(interactor, &call.args);
        }

        match registry.call(&call.name, &call.args, ctx) {
            Ok(output) => output.text,
            Err(error) => format!("{} failed: {error}", call.name),
        }
    }
}

/// The preview an approval view can build from a call's arguments alone.
///
/// `bash` states its whole effect in `command`, so what will run can be shown before it runs.
/// `edit` and `write` only produce a diff by running, so they are approved against their arguments
/// rather than against a diff this function would have to invent — a preview that is not the
/// change is worse than no preview.
fn preview_for(name: &str, args: &serde_json::Value) -> Option<Preview> {
    match name {
        "bash" => args
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(|command| Preview::Command(command.to_string())),
        _ => None,
    }
}
