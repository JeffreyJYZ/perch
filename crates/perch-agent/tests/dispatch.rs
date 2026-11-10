//! Task 13: tool dispatch, approvals, and the question tool.
//!
//! Every test also pins the call id on the tool message it gets back, because the provider
//! addresses each result to the assistant's call by that id and a wrong one is rejected.

use serde_json::{Value, json};

use perch_agent::ToolCall;
use perch_agent::dispatch::{Answer, Decision, Dispatcher, Interactor};
use perch_tools::{Preview, ToolCtx, ToolRegistry};

fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args,
    }
}

fn registry() -> ToolRegistry {
    Dispatcher::builtin_registry()
}

fn ctx(root: &std::path::Path) -> ToolCtx {
    ToolCtx {
        root: root.to_path_buf(),
    }
}

/// An interactor whose every answer is scripted, so a test states the user's behaviour and
/// nothing else.
struct Script {
    /// How many times the dispatcher asked for approval.
    prompts: usize,
    /// What each approval gets: consumed in order, the last one repeating.
    decisions: Vec<Decision>,
    /// The preview each approval was shown.
    previews: Vec<Option<Preview>>,
    answer: Answer,
}

impl Script {
    fn deny_all() -> Self {
        Self {
            prompts: 0,
            decisions: vec![Decision::Deny],
            previews: Vec::new(),
            answer: Answer::Cancelled,
        }
    }

    /// Allow the first prompt for the session, refuse every later one, so a second prompt is
    /// visible both as a count and as a call that did not run.
    fn allow_session_once() -> Self {
        Self {
            prompts: 0,
            decisions: vec![Decision::AllowForSession, Decision::Deny],
            previews: Vec::new(),
            answer: Answer::Cancelled,
        }
    }

    fn answer(answer: Answer) -> Self {
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

#[test]
fn a_denied_tool_never_runs_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::deny_all();
    let out = dispatcher.run(
        &[call("call_1", "bash", json!({"command": "touch pwned"}))],
        &registry(),
        &root,
        &mut interactor,
    );

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].tool_call_id.as_deref(), Some("call_1"));
    let text = out[0].content.clone().unwrap();
    assert!(
        text.contains("denied"),
        "the model must be told the call was denied, got: {text}"
    );
    assert!(
        !dir.path().join("pwned").exists(),
        "a denied call must not have run"
    );
    assert_eq!(
        interactor.previews,
        vec![Some(Preview::Command("touch pwned".to_string()))],
        "the approval view is shown the command before anything runs"
    );
}

#[test]
fn allow_for_session_stops_asking_for_that_tool() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::allow_session_once();

    for (index, id) in ["c1", "c2", "c3"].iter().enumerate() {
        let out = dispatcher.run(
            &[call(id, "bash", json!({"command": "printf ok"}))],
            &registry(),
            &root,
            &mut interactor,
        );
        assert_eq!(out[0].tool_call_id.as_deref(), Some(*id));
        assert!(
            out[0].content.as_deref().unwrap().contains("ok"),
            "run {index} was not allowed to run: {:?}",
            out[0].content
        );
    }

    assert_eq!(
        interactor.prompts, 1,
        "allowing a tool for the session must stop the later prompts"
    );
}

#[test]
fn the_question_tool_returns_the_picked_option_and_free_text() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let registry = registry();

    // `ask_user` is an ordinary tool: registered with a schema, and never gated by approval.
    let names: Vec<String> = registry.specs().into_iter().map(|spec| spec.name).collect();
    assert!(
        names.contains(&"ask_user".to_string()),
        "ask_user must be registered like any other tool, got {names:?}"
    );
    assert!(!registry.needs_approval("ask_user").unwrap());

    let mut interactor = Script::answer(Answer::Pick(1));
    let out = dispatcher.run(
        &[call(
            "q1",
            "ask_user",
            json!({"prompt": "which?", "options": ["a", "b"]}),
        )],
        &registry,
        &root,
        &mut interactor,
    );
    assert_eq!(out[0].tool_call_id.as_deref(), Some("q1"));
    assert_eq!(out[0].content.as_deref(), Some("b"));

    let mut interactor = Script::answer(Answer::Text("neither, use c".into()));
    let out = dispatcher.run(
        &[call(
            "q2",
            "ask_user",
            json!({"prompt": "which?", "options": ["a", "b"]}),
        )],
        &registry,
        &root,
        &mut interactor,
    );
    assert_eq!(out[0].tool_call_id.as_deref(), Some("q2"));
    assert_eq!(out[0].content.as_deref(), Some("neither, use c"));
}

#[test]
fn cancelling_a_question_ends_the_turn_rather_than_answering_for_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::answer(Answer::Cancelled);
    let out = dispatcher.run(
        &[call("q1", "ask_user", json!({"prompt": "which?"}))],
        &registry(),
        &root,
        &mut interactor,
    );

    assert_eq!(out[0].tool_call_id.as_deref(), Some("q1"));
    assert_eq!(
        out[0].content.as_deref(),
        Some("the user cancelled the question")
    );
}

#[test]
fn a_failing_tool_becomes_message_content_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let root = ctx(dir.path());
    let mut dispatcher = Dispatcher::new();
    let mut interactor = Script::answer(Answer::Cancelled);
    let out = dispatcher.run(
        &[
            call("c1", "read", json!({"path": "missing.txt"})),
            call("c2", "no_such_tool", json!({})),
        ],
        &registry(),
        &root,
        &mut interactor,
    );

    assert_eq!(out.len(), 2, "one tool message per call, whatever happened");
    assert_eq!(out[0].tool_call_id.as_deref(), Some("c1"));
    assert_eq!(out[1].tool_call_id.as_deref(), Some("c2"));
    assert!(
        out[0]
            .content
            .as_deref()
            .unwrap()
            .contains("No such file or directory"),
        "the read failure reaches the model as text, got {:?}",
        out[0].content
    );
    assert!(
        out[1].content.as_deref().unwrap().contains("no tool named"),
        "an unknown tool is reported, not panicked on, got {:?}",
        out[1].content
    );
}
