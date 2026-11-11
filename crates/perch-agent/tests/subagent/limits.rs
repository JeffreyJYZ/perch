//! The two caps the tool enforces itself: the brief it accepts and the answer it returns.

use perch_agent::subagent::{PROMPT_MAX_BYTES, RESULT_MAX_BYTES};
use perch_tools::Tool;

use crate::harness::{self, Memory};

#[test]
fn a_prompt_over_the_cap_is_refused_before_the_delegate_runs() {
    let (task, memory) = harness::tool(&harness::parent(), Memory::ok("done"));
    let over = "x".repeat(PROMPT_MAX_BYTES + 1);
    let why = harness::refused(task.call(&harness::args("read a lot", &over), &harness::ctx()));
    assert!(
        why.contains(&PROMPT_MAX_BYTES.to_string()),
        "the refusal does not name the cap: {why}"
    );
    assert_eq!(
        memory.calls(),
        0,
        "the delegate ran for a brief that was refused"
    );
}

#[test]
fn a_prompt_exactly_at_the_cap_is_accepted() {
    let (task, memory) = harness::tool(&harness::parent(), Memory::ok("done"));
    let at = "x".repeat(PROMPT_MAX_BYTES);
    let out = task
        .call(&harness::args("read a lot", &at), &harness::ctx())
        .expect("a brief at the cap is inside it");
    assert!(
        out.text.starts_with("task \"read a lot\" -> done"),
        "{}",
        out.text
    );
    assert_eq!(memory.calls(), 1);
}

#[test]
fn an_answer_over_the_cap_is_cut_with_the_cut_named() {
    let long = "y".repeat(RESULT_MAX_BYTES + 512);
    let (task, _) = harness::tool(&harness::parent(), Memory::ok(&long));
    let text = task
        .call(&harness::args("read a lot", "read it"), &harness::ctx())
        .expect("a long answer is still a result")
        .text;
    assert!(
        text.starts_with("task \"read a lot\" -> done\n\n"),
        "the heading was lost to the cut: {text}"
    );
    assert!(
        text.contains("[truncated: the child answered"),
        "the cut was not named: {text}"
    );
    assert!(
        text.len() < long.len(),
        "the cut kept every byte: {} bytes",
        text.len()
    );
}
