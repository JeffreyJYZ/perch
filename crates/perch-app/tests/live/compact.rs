//! `/compact` at the app's seam: taken while idle and while a turn runs, ahead of a queued prompt.

use perch_ui::Item;

use crate::harness::{TIMEOUT, app_against};

/// Every note the column says, in order.
fn notes(app: &perch_app::AppState) -> Vec<String> {
    app.active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Note(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The index of the first item the column holds that matches `wanted`.
fn position(app: &perch_app::AppState, wanted: impl Fn(&Item) -> bool) -> Option<usize> {
    app.active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .position(wanted)
}

/// Runs `turns` turns of the repeating fixture, one prompt each.
fn run_turns(app: &mut perch_app::AppState, turns: u32) {
    for index in 1..=turns {
        app.prompt(format!("turn {index}")).unwrap();
        assert!(
            app.pump_until(TIMEOUT, |session| session.turns_ended() == index),
            "turn {index} should end within {TIMEOUT:?}"
        );
    }
}

#[test]
fn an_idle_compaction_runs_at_once_and_says_how_many_were_left_out() {
    let (mut app, _server, _data, _worktree) = app_against("tests/fixtures/live_repeat.txt");
    run_turns(&mut app, 2);

    app.compact().unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| {
            session
                .transcript()
                .items()
                .iter()
                .any(|item| matches!(item, Item::Note(text) if text.contains("compacted:")))
        }),
        "an idle compaction answers without a turn: {TIMEOUT:?}"
    );

    assert!(
        notes(&app)
            .iter()
            .any(|note| note == "compacted: 1 older exchange (2 messages) left out."),
        "the column says how many messages went: {:?}",
        notes(&app)
    );
    assert_eq!(
        app.active_session().unwrap().turns_ended(),
        2,
        "a compaction is not a turn, so it ends none"
    );
}

#[test]
fn a_session_with_nothing_to_compact_refuses_in_words() {
    let (mut app, _server, _data, _worktree) = app_against("tests/fixtures/live_repeat.txt");
    run_turns(&mut app, 1);

    app.compact().unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| {
            session
                .transcript()
                .items()
                .iter()
                .any(|item| matches!(item, Item::Note(text) if text.contains("nothing to compact")))
        }),
        "a refusal is words in the column: {:?}",
        notes(&app)
    );
}

#[test]
fn a_compaction_typed_mid_turn_runs_before_the_prompt_queued_behind_it() {
    let (mut app, _server, _data, _worktree) = app_against("tests/fixtures/live_queue.txt");
    run_turns(&mut app, 2);

    // The third turn is in flight; a prompt is held, and a compaction is asked for after it.
    app.prompt("turn 3").unwrap();
    app.prompt("after compacting").unwrap();
    app.compact().unwrap();
    assert!(
        app.pump_until(TIMEOUT, |session| session.turns_ended() == 4),
        "the held prompt should still run: {TIMEOUT:?}"
    );

    let compacted = position(
        &app,
        |item| matches!(item, Item::Note(text) if text.starts_with("compacted:")),
    )
    .unwrap_or_else(|| panic!("the compaction was never said: {:?}", notes(&app)));
    let held = position(
        &app,
        |item| matches!(item, Item::User { text } if text == "after compacting"),
    )
    .expect("the held prompt went out");
    let third = app
        .active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .enumerate()
        .filter(|(_, item)| matches!(item, Item::Assistant { .. }))
        .nth(2)
        .map(|(index, _)| index)
        .expect("three turns answered");
    assert!(
        compacted > third,
        "the compaction ran at the end of the running turn, not inside it"
    );
    assert!(
        compacted < held,
        "the compaction ran ahead of the prompt queued behind it"
    );
    assert!(
        notes(&app)
            .iter()
            .any(|note| note == "compacted: 1 older exchange (2 messages) left out."),
        "the second turn's exchange is what went: {:?}",
        notes(&app)
    );
}
