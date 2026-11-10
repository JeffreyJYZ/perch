//! The live registry: a call keeps the tool list it started with, and the next call sees the new
//! one. No turn is rewound and no message is edited — the change is deferred to a call boundary.

use std::path::PathBuf;

use perch_agent::{Answer, Decision, Dispatcher, Interactor, ToolCall};
use perch_app::Live;
use perch_tools::{Preview, Result, Tool, ToolCtx, ToolOutput, ToolRegistry, ToolSpec};
use serde_json::json;

/// A tool that answers with a fixed word, so two registries are told apart by what they return.
struct Echo(&'static str);

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".to_string(),
            description: "answer with this registry's word".to_string(),
            parameters: json!({"type": "object", "properties": {}}),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, _args: &serde_json::Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        Ok(ToolOutput::text(self.0))
    }
}

/// A registry holding only `echo`, answering with `word`.
fn registry(word: &'static str) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(std::sync::Arc::new(Echo(word)));
    registry
}

fn ctx() -> ToolCtx {
    ToolCtx {
        root: PathBuf::from("."),
    }
}

fn call() -> ToolCall {
    ToolCall {
        id: "call_1".to_string(),
        name: "echo".to_string(),
        args: json!({}),
    }
}

/// Never asked: `echo` needs no approval and asks no question.
struct Nobody;

impl Interactor for Nobody {
    fn approve(&mut self, _name: &str, _preview: Option<&Preview>) -> Decision {
        Decision::Deny
    }

    fn ask(&mut self, _prompt: &str, _options: &[String], _multiple: bool) -> Answer {
        Answer::Cancelled
    }
}

#[test]
fn a_call_already_holding_the_old_list_finishes_against_it() {
    let live = Live::new(registry("old"));
    // A call that has begun: this is the guard the dispatcher holds for its whole run.
    let in_flight = live.load();

    live.store(registry("new"));

    assert_eq!(
        in_flight.call("echo", &json!({}), &ctx()).unwrap().text,
        "old",
        "a reload reached a call that was already running"
    );
    assert_eq!(
        live.load().call("echo", &json!({}), &ctx()).unwrap().text,
        "new",
        "the next call did not see the reloaded tools"
    );
}

#[test]
fn a_reload_lands_on_the_next_call_of_the_same_turn() {
    let live = Live::new(registry("old"));
    let mut dispatcher = Dispatcher::new();
    let mut nobody = Nobody;

    let before = {
        let registry = live.load();
        dispatcher.run(&[call()], &registry, &ctx(), &mut nobody)
    };
    // The turn is still open: only the tool list changed.
    live.store(registry("new"));
    let after = {
        let registry = live.load();
        dispatcher.run(&[call()], &registry, &ctx(), &mut nobody)
    };

    assert_eq!(before[0].message.content.as_deref(), Some("old"));
    assert_eq!(
        after[0].message.content.as_deref(),
        Some("new"),
        "the second call of the turn did not see the new tools"
    );
    assert_eq!(after[0].message.tool_call_id.as_deref(), Some("call_1"));
}

#[test]
fn a_tool_that_is_gone_is_no_longer_offered_or_dispatched() {
    let live = Live::new(registry("old"));
    assert_eq!(live.names(), vec!["echo".to_string()]);

    live.store(ToolRegistry::new());

    assert!(live.names().is_empty(), "{}", live.names().len());
    assert!(
        live.call("echo", &json!({}), &ctx()).is_err(),
        "a removed tool still dispatches"
    );
}
