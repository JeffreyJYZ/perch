//! Tool dispatch: the approval gate, execution, and one tool message per call.
//!
//! The dispatcher is the only place a tool the model asked for becomes a result. It consults the
//! registry for whether a call needs approval, asks the [`Interactor`], runs the call, and answers
//! every call with a [`ToolReply`]: a [`Message::tool_result`] that repeats the call's id — the
//! provider addresses each result to the assistant's call by that id, so a lost or renumbered id
//! is a rejected conversation — plus whether the call actually ran.
//!
//! Nothing here can panic into the turn: a tool that fails, an unknown tool, and a denied call all
//! become message content, because the model can act on a stated failure and cannot act on a
//! dead thread. The outcome travels beside that text so no caller has to read the wording to
//! learn whether the call ran.

use std::collections::BTreeSet;
use std::sync::Arc;

use perch_tools::{Preview, ToolCtx, ToolRegistry};

use crate::ask::{self, AskUser};
use crate::event::ToolCall;
use crate::provider::Message;

/// One call's answer: the tool message the provider receives, and whether the call ran.
///
/// `ok` is the outcome, not the tool's opinion of it. A command that exits non-zero still ran, so
/// it is `ok: true` and the model reads the exit code; a denial, a failed preview, an unknown tool
/// and a tool that returned an error are `ok: false`, because none of them produced a result.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolReply {
    /// The `tool` message that answers the call, carrying the call's id.
    pub message: Message,
    /// Whether the call ran.
    pub ok: bool,
}

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
    /// Picked these options, in the order they were picked: one index for a single-choice
    /// question, several for `multiple: true`, and none for a dismissal.
    Picks(Vec<usize>),
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
    ) -> Vec<ToolReply> {
        calls
            .iter()
            .map(|call| self.one(call, registry, ctx, interactor))
            .collect()
    }

    /// The reply one call answers with, for good or ill.
    fn one(
        &mut self,
        call: &ToolCall,
        registry: &ToolRegistry,
        ctx: &ToolCtx,
        interactor: &mut dyn Interactor,
    ) -> ToolReply {
        match registry.needs_approval(&call.name) {
            Ok(false) => {}
            Ok(true) => {
                // A tool already allowed for the session is not asked about again; the allowance
                // is keyed by name, so allowing `bash` says nothing about `write`.
                if !self.allowed.contains(&call.name) {
                    // The tool describes its own change, so nothing here has to know what a diff
                    // is. A change that cannot be described is denied rather than approved
                    // blind, and it is denied *before* the user is asked: offering a call whose
                    // preview failed would ask them to approve something nobody can see.
                    let preview = match registry.preview(&call.name, &call.args, ctx) {
                        Ok(preview) => preview,
                        Err(error) => {
                            return reply(
                                call,
                                format!(
                                    "the {} call was not run because its preview failed: {error}",
                                    call.name
                                ),
                                false,
                            );
                        }
                    };
                    match interactor.approve(&call.name, preview.as_ref()) {
                        Decision::Allow => {}
                        Decision::AllowForSession => {
                            self.allowed.insert(call.name.clone());
                        }
                        Decision::Deny => {
                            return reply(
                                call,
                                format!(
                                    "the user denied the {} call, so it did not run",
                                    call.name
                                ),
                                false,
                            );
                        }
                    }
                }
            }
            // An unknown tool is refused before anything runs, which also keeps the question
            // bridge below from asking a user about a call that cannot be made.
            Err(error) => return reply(call, error.to_string(), false),
        }

        if call.name == ask::ASK_USER {
            ask::answer_through(interactor, &call.args);
        }

        match registry.call(&call.name, &call.args, ctx) {
            // The command's own exit status is part of its output, so a non-zero exit is a
            // result the model must see, not a failure of the call.
            Ok(output) => reply(call, output.text, true),
            Err(error) => reply(call, format!("{} failed: {error}", call.name), false),
        }
    }
}

/// One tool message for `call`, with the outcome beside it.
fn reply(call: &ToolCall, text: String, ok: bool) -> ToolReply {
    ToolReply {
        message: Message::tool_result(call.id.clone(), text),
        ok,
    }
}
