//! The column a reopen rebuilds: its rows in order, a cancelled turn's own word, and what it says
//! about the parts of the log it could not keep.

use perch_store::SessionEvent;

use crate::harness::{damage, exchange, log, notes, open, place, shape, users};

#[test]
fn two_exchanges_reopen_into_their_rows_in_order() {
    let (data, worktree) = place();
    log(
        data.path(),
        worktree.path(),
        &[
            exchange("how do I build?", "cargo build."),
            exchange("and test?", "cargo test."),
        ]
        .concat(),
    );
    let (app, _server) = open(data.path(), "tests/fixtures/live_text.txt");

    assert_eq!(
        shape(&app),
        vec![
            "user: how do I build?",
            "assistant: cargo build.",
            "summary: 0 tools, 7 tokens",
            "user: and test?",
            "assistant: cargo test.",
            "summary: 0 tools, 7 tokens",
        ],
        "the log replays as the column showed it"
    );
    assert!(
        notes(&app).is_empty(),
        "nothing was left out, so nothing is said: {:?}",
        notes(&app)
    );
}

#[test]
fn a_tool_call_reopens_with_its_status_and_its_output() {
    let (data, worktree) = place();
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
                text: "fn main() {}".to_string(),
            },
            SessionEvent::Assistant {
                text: "It starts at main.".to_string(),
            },
            SessionEvent::TurnEnd {
                reason: "done".to_string(),
                tokens_in: 9,
                tokens_out: 4,
            },
        ],
    );
    let (app, _server) = open(data.path(), "tests/fixtures/live_text.txt");

    assert_eq!(
        shape(&app),
        vec![
            "user: read it",
            "tool: read done fn main() {}",
            "assistant: It starts at main.",
            "summary: 1 tools, 13 tokens",
        ]
    );
}

#[test]
fn a_cancelled_turn_reopens_with_the_word_it_showed_live() {
    let (data, worktree) = place();
    log(
        data.path(),
        worktree.path(),
        &[
            SessionEvent::User {
                text: "count to ten".to_string(),
            },
            SessionEvent::Assistant {
                text: "one, two".to_string(),
            },
            SessionEvent::TurnEnd {
                reason: "cancelled".to_string(),
                tokens_in: 3,
                tokens_out: 2,
            },
        ],
    );
    let (app, _server) = open(data.path(), "tests/fixtures/live_text.txt");

    assert_eq!(
        shape(&app),
        vec![
            "user: count to ten",
            "assistant: one, two",
            "note: The turn was cancelled.",
            "summary: 0 tools, 5 tokens",
        ],
        "a turn that was stopped comes back stopped, never failed"
    );
}

#[test]
fn a_log_longer_than_the_cap_reopens_the_last_exchanges_and_says_once_how_many_went() {
    let (data, worktree) = place();
    let total = perch_app::MAX_EXCHANGES + 3;
    let long = (1..=total)
        .flat_map(|turn| exchange(&format!("turn {turn}"), &format!("answer {turn}")))
        .collect::<Vec<_>>();
    log(data.path(), worktree.path(), &long);
    let (app, _server) = open(data.path(), "tests/fixtures/live_text.txt");

    let first = total - perch_app::MAX_EXCHANGES + 1;
    let kept = (first..=total)
        .map(|turn| format!("turn {turn}"))
        .collect::<Vec<_>>();
    assert_eq!(users(&app), kept, "the newest exchanges are the ones kept");

    assert_eq!(
        notes(&app),
        vec![
            "reopened: 3 older exchanges (6 messages) left out; the log still has them."
                .to_string()
        ],
        "the column says once what it could not carry"
    );
}

#[test]
fn a_corrupt_line_is_skipped_counted_and_the_rest_still_replays() {
    let (data, worktree) = place();
    log(
        data.path(),
        worktree.path(),
        &[exchange("first", "one"), exchange("second", "two")].concat(),
    );
    damage(data.path(), "{\"kind\":\"assis");
    let (app, _server) = open(data.path(), "tests/fixtures/live_text.txt");

    assert_eq!(
        notes(&app),
        vec!["reopened: 1 line of the log could not be read and was skipped.".to_string()],
        "a torn line is counted, not hidden"
    );
    assert_eq!(users(&app), vec!["first", "second"]);
    assert!(
        shape(&app).contains(&"assistant: two".to_string()),
        "the exchanges either side of the damage still replay: {:?}",
        shape(&app)
    );
}
