//! Rules a person saved: they outlive the session, they can be undone, and the file is private.

use std::path::Path;

use serde_json::json;

use perch_agent::dispatch::{Answer, Decision, Dispatcher, Interactor};
use perch_agent::{Permission, Rules, ToolCall};
use perch_tools::{Preview, ToolCtx};

use crate::harness::at;

#[test]
fn a_remembered_allow_applies_on_the_next_call_across_a_reopened_store() {
    let tmp = tempfile::tempdir().unwrap();
    let mut rules = at(tmp.path());
    rules
        .remember(
            "bash",
            &json!({ "command": "cargo test" }),
            Decision::AllowForSession,
        )
        .unwrap();
    drop(rules);

    let rules = at(tmp.path());
    let root = tmp.path();
    assert_eq!(
        rules.permit("bash", &json!({ "command": "cargo test" }), root),
        Some(Permission::Allow),
        "the rule outlives the session that saved it"
    );
    assert_eq!(
        rules.permit("bash", &json!({ "command": "cargo test --all" }), root),
        None,
        "an exact command rule covers only that command"
    );
}

#[test]
fn forget_removes_one_tool_rules_and_leaves_the_rest() {
    let tmp = tempfile::tempdir().unwrap();
    let mut rules = at(tmp.path());
    rules
        .remember("bash", &json!({ "command": "true" }), Decision::Allow)
        .unwrap();
    rules
        .remember("write", &json!({ "path": "src/" }), Decision::Allow)
        .unwrap();

    rules.forget("bash").unwrap();

    let rules = at(tmp.path());
    let root = tmp.path();
    assert_eq!(
        rules.permit("bash", &json!({ "command": "true" }), root),
        None
    );
    assert!(
        rules
            .permit("write", &json!({ "path": "src/x" }), root)
            .is_some()
    );
    let text = std::fs::read_to_string(root.join("permissions.jsonl")).unwrap();
    assert!(
        !text.contains("bash"),
        "the file no longer carries it: {text}"
    );
}

#[cfg(unix)]
#[test]
fn the_rules_file_is_private() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let mut rules = at(tmp.path());
    rules
        .remember("bash", &json!({ "command": "true" }), Decision::Allow)
        .unwrap();

    let file = std::fs::metadata(tmp.path().join("permissions.jsonl")).unwrap();
    assert_eq!(file.permissions().mode() & 0o777, 0o600);
}

/// A person who denies everything and counts how often they were asked.
struct Counted {
    prompts: usize,
}

impl Interactor for Counted {
    fn approve(&mut self, _name: &str, _preview: Option<&Preview>) -> Decision {
        self.prompts += 1;
        Decision::Deny
    }

    fn ask(&mut self, _prompt: &str, _options: &[String], _multiple: bool) -> Answer {
        Answer::Cancelled
    }
}

/// Runs one `bash true` through a dispatcher that reads the rules at `dir`.
fn run(dir: &Path) -> (usize, bool, Option<String>) {
    let mut dispatcher = Dispatcher::new().with_rules(at(dir));
    let registry = Dispatcher::builtin_registry();
    let ctx = ToolCtx {
        root: dir.to_path_buf(),
    };
    let mut user = Counted { prompts: 0 };
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "bash".to_string(),
        args: json!({ "command": "true" }),
    };
    let reply = dispatcher
        .run(&[call], &registry, &ctx, &mut user)
        .remove(0);
    (user.prompts, reply.ok, reply.message.content)
}

fn save(dir: &Path, decision: Decision) -> Rules {
    let mut rules = at(dir);
    rules
        .remember("bash", &json!({ "command": "true" }), decision)
        .unwrap();
    rules
}

#[test]
fn a_saved_decision_answers_instead_of_asking() {
    let allowed = tempfile::tempdir().unwrap();
    save(allowed.path(), Decision::Allow);
    let (prompts, ok, _) = run(allowed.path());
    assert_eq!(prompts, 0, "the saved allow answered for the call");
    assert!(ok, "and the call ran");

    let denied = tempfile::tempdir().unwrap();
    save(denied.path(), Decision::Deny);
    let (prompts, ok, text) = run(denied.path());
    assert_eq!(prompts, 0);
    assert!(!ok, "a saved deny is not a result");
    assert!(
        text.is_some_and(|text| text.contains("saved permission rule denies")),
        "the model is told why it did not run"
    );
}
