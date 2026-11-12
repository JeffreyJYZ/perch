//! `/diff` on a turn that changed something: the set, and each file's own diff.
//!
//! The fixtures in this file are shared with `diff_refusals`, which is why they are public. Every
//! repository is made and thrown away inside its test, and every turn is folded into a transcript
//! from the agent's own events, so nothing reads the worktree the suite runs in.

use std::path::Path;
use std::process::Command;

use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};
use perch_ui::Shell;
use perch_ui::Transcript;
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::harness::{Fake, notes};

/// Runs `git` in `dir`, with identity and signing forced off so no global config decides it.
pub fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.email=perch-test@example.com",
            "-c",
            "user.name=perch test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

/// A repository with `files` committed, so writing one afterwards is a real change.
pub fn repo(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--quiet"]);
    write(dir.path(), files);
    git(dir.path(), &["add", "--all"]);
    git(dir.path(), &["commit", "--quiet", "-m", "fixture"]);
    dir
}

/// Writes `files` under `dir`, creating their directories.
pub fn write(dir: &Path, files: &[(&str, &str)]) {
    for (path, contents) in files {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }
}

/// A path argument, the way a writing tool names it.
pub fn path(path: &str) -> Value {
    json!({ "path": path })
}

/// One finished turn that ran `calls`, on `model`.
pub fn turn(model: &mut Transcript, calls: &[(&str, Value)]) {
    for (index, (name, args)) in calls.iter().enumerate() {
        let id = format!("call-{index}");
        model.apply(&AgentEvent::ToolCall(ToolCall {
            id: id.clone(),
            name: (*name).to_string(),
            args: args.clone(),
        }));
        model.apply(&AgentEvent::ToolResult {
            id,
            ok: true,
            text: String::new(),
        });
    }
    model.apply(&AgentEvent::TurnEnd {
        reason: EndReason::Done,
        usage: Usage::default(),
    });
}

/// A window over a session on `dir`, with `calls` as the turn that just finished.
pub fn shell_on(dir: &Path, calls: &[(&str, Value)]) -> Shell<Fake> {
    let mut fake = Fake::one("one", dir);
    turn(&mut fake.transcript, calls);
    Shell::new(fake.metas.clone(), 0).with_live(fake)
}

/// Types `text` and presses Enter, as the composer does.
pub fn run(shell: &mut Shell<Fake>, text: &str) {
    shell.set_prompt(text);
    assert!(shell.submit(), "{text} is a command and is taken");
}

/// A repository with two edits the turn made, and one file it did not touch.
pub fn changed_twice() -> TempDir {
    let dir = repo(&[("a.txt", "one\n"), ("b.txt", "two\n"), ("c.txt", "three\n")]);
    write(
        dir.path(),
        &[("a.txt", "one\nmore\n"), ("b.txt", "two\nalso\n")],
    );
    dir
}

#[test]
fn the_files_the_last_turn_wrote_and_each_one_s_diff() {
    let dir = changed_twice();
    let mut shell = shell_on(
        dir.path(),
        &[("edit", path("a.txt")), ("write", path("b.txt"))],
    );

    run(&mut shell, "/diff");

    let said = notes(&shell);
    assert!(
        said.iter()
            .any(|note| note == "diff: 2 file(s) changed by the last turn: a.txt, b.txt"),
        "the set is named first: {said:?}"
    );
    assert!(
        said.iter()
            .any(|note| note.contains("--- a.txt") && note.contains("+more")),
        "a.txt's own diff is shown: {said:?}"
    );
    assert!(
        said.iter()
            .any(|note| note.contains("--- b.txt") && note.contains("+also")),
        "and b.txt's: {said:?}"
    );
}

#[test]
fn a_turn_that_changed_nothing_says_so() {
    let dir = changed_twice();
    let mut shell = shell_on(
        dir.path(),
        &[
            ("read", path("a.txt")),
            ("bash", json!({ "command": "cargo test" })),
        ],
    );

    run(&mut shell, "/diff");

    let said = notes(&shell);
    assert!(
        said.iter()
            .any(|note| note == "diff: the last turn changed no file"),
        "a turn that wrote nothing is one sentence, not an empty box: {said:?}"
    );
}

#[test]
fn naming_one_file_narrows_to_it() {
    let dir = changed_twice();
    let mut shell = shell_on(
        dir.path(),
        &[("edit", path("a.txt")), ("write", path("b.txt"))],
    );

    run(&mut shell, "/diff a.txt");

    let said = notes(&shell);
    assert!(
        said.iter().any(|note| note.contains("--- a.txt")),
        "the named file's diff: {said:?}"
    );
    assert!(
        !said.iter().any(|note| note.contains("--- b.txt")),
        "and nothing of the other: {said:?}"
    );
}
