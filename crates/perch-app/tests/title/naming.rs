//! Naming a session after its first turn, and every case the contract names.

use std::sync::atomic::Ordering;

use perch_ui::Item;

use crate::harness::{SETTLE, app_against, laid_out, log_notes, settle, sidebar, titled, turn};

#[test]
fn the_first_turn_names_the_session_and_the_sidebar_reads_it() {
    let (mut app, server, _data, _root, _work) = app_against("tests/fixtures/live_title.txt");
    turn(&mut app, "what is this?");
    assert!(
        titled(&mut app, "Fix the terminal grid"),
        "the title landed"
    );
    assert_eq!(app.metas()[0].title, "Fix the terminal grid");
    assert_eq!(
        server.requests.load(Ordering::SeqCst),
        2,
        "one turn and one title request"
    );

    let runtime = laid_out(app);
    let words = sidebar(&runtime);
    assert!(words.contains("Fix the terminal grid"), "{words}");
    assert!(
        words.contains("work"),
        "the worktree stays visible under the title: {words}"
    );
}

#[test]
fn a_manual_rename_wins_and_the_provider_is_never_asked() {
    let (mut app, server, _data, _root, _work) = app_against("tests/fixtures/live_title.txt");
    app.rename_session(0, "Mine").unwrap();
    turn(&mut app, "what is this?");
    settle(&mut app, SETTLE);

    assert_eq!(app.metas()[0].title, "Mine");
    assert_eq!(
        server.requests.load(Ordering::SeqCst),
        1,
        "only the turn was sent"
    );
}

#[test]
fn a_failure_leaves_the_folder_name_and_only_writes_a_log_line() {
    let (mut app, server, _data, _root, _work) = app_against("tests/fixtures/live_title_fail.txt");
    turn(&mut app, "what is this?");
    let deadline = std::time::Instant::now() + SETTLE;
    while app.titles_pending() > 0 && std::time::Instant::now() < deadline {
        app.pump(std::time::Duration::from_millis(5));
    }
    settle(&mut app, SETTLE);

    assert_eq!(app.metas()[0].title, "work", "the folder name stays");
    let notes = log_notes(&app);
    assert!(
        notes.iter().any(|note| note.starts_with("title:")),
        "the log says the title failed: {notes:?}"
    );
    let transcript = app.active_session().unwrap().transcript();
    assert!(
        transcript
            .items()
            .iter()
            .all(|item| !matches!(item, Item::Note(_))),
        "the transcript says nothing about a title"
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), 2);
}

#[test]
fn a_second_turn_does_not_issue_a_second_title_request() {
    let (mut app, server, _data, _root, _work) = app_against("tests/fixtures/live_title_twice.txt");
    turn(&mut app, "what is this?");
    assert!(titled(&mut app, "Fix the terminal grid"));

    turn(&mut app, "and again?");
    settle(&mut app, SETTLE);
    assert_eq!(
        server.requests.load(Ordering::SeqCst),
        3,
        "the turn, the title, and the second turn: no third title request"
    );
    assert_eq!(app.metas()[0].title, "Fix the terminal grid");
}

#[test]
fn a_long_reply_is_cut_to_six_words_on_one_line_and_the_prompt_is_the_contract() {
    let (mut app, server, _data, _root, _work) = app_against("tests/fixtures/live_title_long.txt");
    turn(&mut app, "what is this?");
    assert!(
        titled(&mut app, "one two three four five six"),
        "eight words became six: {:?}",
        app.metas()[0].title
    );
    let title = app.metas()[0].title.clone();
    assert!(!title.contains('\n'));
    assert!(!title.ends_with('.'));

    let bodies = server.bodies.lock().unwrap();
    let title_request = &bodies[1];
    assert!(
        title_request.contains("at most six words"),
        "the system line is the one this lane wrote: {title_request}"
    );
    assert!(title_request.contains("Question:"));
    assert!(title_request.contains("Write the title now."));
    assert!(
        !title_request.contains("\"tools\""),
        "a title request offers no tools: {title_request}"
    );
}
