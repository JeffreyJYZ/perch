//! The `ask_user` tool: the model's question, and the user's answer as tool output.
//!
//! A question is not a special kind of call. `ask_user` is an ordinary [`Tool`] with a schema,
//! registered beside the six built-ins, so the model sees it the way it sees `read` and a plugin
//! could take the same path. Only the answering is special: [`Tool::call`] takes `&self` and has
//! no room for an interactor, so [`Dispatcher::run`](crate::dispatch::Dispatcher::run) asks the
//! user and leaves the answer in a slot scoped to the thread that dispatched the call, and the
//! tool reads it back. Nothing in perch-tools learns that questions exist.
//!
//! A cancelled question answers with a phrase rather than an error, because the model can carry
//! on with a stated fact ("the user cancelled") and cannot carry on with a failure it might
//! decide to retry by inventing an answer.

use std::cell::RefCell;

use serde_json::{Value, json};

use perch_tools::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec};

use crate::dispatch::{Answer, Interactor};

/// The tool name the model calls.
pub const ASK_USER: &str = "ask_user";

/// What the model is told when the user dismissed the question.
pub const CANCELLED: &str = "the user cancelled the question";

thread_local! {
    /// The answer one dispatch is carrying into [`AskUser::call`]. Thread-scoped because that is
    /// the scope of a dispatch: nothing crosses a thread and nothing leaks between sessions.
    static PENDING: RefCell<Option<Answer>> = const { RefCell::new(None) };
}

/// Leaves the user's answer where [`AskUser`] will find it.
pub(crate) fn offer(answer: Answer) {
    PENDING.with(|slot| *slot.borrow_mut() = Some(answer));
}

fn take() -> Option<Answer> {
    PENDING.with(|slot| slot.borrow_mut().take())
}

/// One `ask_user` call's arguments.
///
/// Parsed in one place so the dispatcher asks exactly the question the tool then answers.
pub(crate) struct Question {
    pub prompt: String,
    pub options: Vec<String>,
    pub multiple: bool,
}

pub(crate) fn parse_question(args: &Value) -> Result<Question> {
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Message("missing string argument `prompt`".to_string()))?
        .to_string();
    let mut options = Vec::new();
    if let Some(list) = args.get("options") {
        let list = list
            .as_array()
            .ok_or_else(|| Error::Message("`options` must be an array of strings".to_string()))?;
        for option in list {
            let option = option.as_str().ok_or_else(|| {
                Error::Message("`options` must be an array of strings".to_string())
            })?;
            options.push(option.to_string());
        }
    }
    let multiple = args
        .get("multiple")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(Question {
        prompt,
        options,
        multiple,
    })
}

/// `ask_user`: put a question to the user and return what the user did.
pub struct AskUser;

impl Tool for AskUser {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: ASK_USER.into(),
            description: "Ask the user a question and wait for the answer. Give options when the \
                          answer is a choice; the user may still type free text."
                .into(),
            parameters: json!({
                "type": "object",
                "description": "Ask the user one question.",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The question to show the user."
                    },
                    "options": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Choices the user can pick from, in order."
                    },
                    "multiple": {
                        "type": "boolean",
                        "description": "Whether more than one option may be picked."
                    }
                },
                "required": ["prompt"],
                "additionalProperties": false,
            }),
        }
    }

    /// A question changes nothing on disk, so it never asks for approval first.
    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let question = parse_question(args)?;
        let answer = take().ok_or_else(|| {
            Error::Message(format!(
                "{ASK_USER} was called without a dispatcher, so nobody was asked"
            ))
        })?;
        match answer {
            Answer::Pick(index) => {
                let picked = question.options.get(index).ok_or_else(|| {
                    Error::Message(format!(
                        "the answer picked option {index}, but the question offered {}",
                        question.options.len()
                    ))
                })?;
                Ok(ToolOutput::text(picked.clone()))
            }
            Answer::Text(text) => Ok(ToolOutput::text(text)),
            Answer::Cancelled => Ok(ToolOutput::text(CANCELLED)),
        }
    }
}

/// Asks the interactor the question a call states, and leaves the answer for the tool to return.
///
/// Argument problems are left to the tool, so there is exactly one report of a malformed call.
pub(crate) fn answer_through(interactor: &mut dyn Interactor, args: &Value) {
    if let Ok(question) = parse_question(args) {
        let answer = interactor.ask(&question.prompt, &question.options, question.multiple);
        offer(answer);
    }
}
