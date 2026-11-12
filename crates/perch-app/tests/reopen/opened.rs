//! An opened card on a session read back from its log: the same item, replayed, opening the same
//! way — nothing is re-read from the provider, and no second copy of the output exists.

use perch_store::SessionEvent;
use perch_ui::{Item, Transcript, transcript_view};
use rune::{Cx, El, Env, Runtime};

use crate::harness::{log, open, place};

/// `lines` numbered lines, each with its newline: longer than a collapsed card shows.
fn numbered(lines: usize) -> String {
    (1..=lines).map(|n| format!("line {n}\n")).collect()
}

/// The column laid out, as the words a reader can reach.
fn rendered(transcript: Transcript) -> String {
    fn view(model: &Transcript, _: &mut Cx<Transcript>) -> El {
        transcript_view(model)
    }
    let mut runtime = Runtime::new(transcript, view, (900.0, 700.0), Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime.accessibility_json().to_string()
}

#[test]
fn a_reopened_card_opens_the_whole_output_the_log_recorded() {
    let (data, worktree) = place();
    let output = numbered(40);
    log(
        data.path(),
        worktree.path(),
        &[
            SessionEvent::User {
                text: "read it".to_string(),
            },
            SessionEvent::ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                args: serde_json::json!({ "path": "src/main.rs" }),
            },
            SessionEvent::ToolResult {
                id: "call-1".to_string(),
                ok: true,
                text: output.clone(),
            },
            SessionEvent::TurnEnd {
                reason: "done".to_string(),
                tokens_in: 9,
                tokens_out: 4,
            },
        ],
    );
    let (app, _server) = open(data.path(), "tests/fixtures/live_text.txt");
    let mut transcript = app
        .active_session()
        .expect("the log opened a session")
        .transcript()
        .clone();

    let card = transcript
        .items()
        .iter()
        .find(|item| matches!(item, Item::Tool { .. }))
        .expect("the log replays the call as a card");
    assert!(
        matches!(card, Item::Tool { output: recorded, .. } if recorded == &output),
        "the replayed card holds the whole recorded output, not a card-sized copy"
    );
    assert_eq!(
        card.key(1),
        "tool-1",
        "and it is drawn under the column's key"
    );

    let closed = rendered(transcript.clone());
    assert!(
        closed.contains("showing first 5") && !closed.contains("line 6"),
        "a reopened card starts collapsed, as a live one does: {closed}"
    );

    assert!(
        transcript.toggle("call-1"),
        "the same handle opens a replayed card"
    );
    let opened = rendered(transcript.clone());
    assert!(
        opened.contains("line 40") && !opened.contains("showing first 5"),
        "the whole output is drawn once opened: {opened}"
    );
    assert!(
        opened.contains("40 lines"),
        "and the opened block names the size it read back: {opened}"
    );
}

#[test]
fn a_replayed_card_with_no_output_says_so_instead_of_opening() {
    let (data, worktree) = place();
    log(
        data.path(),
        worktree.path(),
        &[
            SessionEvent::User {
                text: "run it".to_string(),
            },
            SessionEvent::ToolCall {
                id: "call-1".to_string(),
                name: "bash".to_string(),
                args: serde_json::json!({ "command": "true" }),
            },
        ],
    );
    let (app, _server) = open(data.path(), "tests/fixtures/live_text.txt");
    let mut transcript = app
        .active_session()
        .expect("the log opened a session")
        .transcript()
        .clone();

    assert!(
        !transcript.toggle("call-1"),
        "a call the log never saw a result for has nothing to open"
    );
    assert!(!transcript.is_expanded("call-1"));
    let tree = rendered(transcript);
    assert!(
        tree.contains("no output yet"),
        "and the card says why in words: {tree}"
    );
}
