//! The list read from disk, and what selecting a row does to the window and the app.

use perch_app::AppState;

use crate::harness::{click, defaults, fixture, laid_out, session_key, sidebar};

#[test]
fn sessions_on_disk_are_listed_and_a_row_switches_the_window_and_the_app() {
    let (data, roots, one, two) = fixture();
    {
        let mut app = AppState::open_with(data.path(), defaults()).unwrap();
        app.new_session(&one).unwrap();
        app.new_session(&two).unwrap();
    }
    // A second open is what reads the list from the session logs, not from this test's memory.
    let app = AppState::open_with(data.path(), defaults()).unwrap();
    assert_eq!(app.metas().len(), 2, "both sessions were on disk");

    let mut runtime = laid_out(app);
    let mut titles = runtime.state.tab_titles();
    titles.sort();
    assert_eq!(
        titles,
        ["one".to_string(), "two".to_string()],
        "the sidebar lists what was read from disk"
    );
    let shown = sidebar(&runtime);
    for title in ["one", "two"] {
        assert!(
            shown.contains(title),
            "the row for {title} is drawn: {shown}"
        );
    }

    // A row's press is the same state swap the tab strip makes. The rows are found by the title
    // they carry, because two sessions made in one second share a creation time.
    let two_row = session_key(&runtime, "two");
    click(&mut runtime, &two_row);
    assert_eq!(
        runtime
            .state
            .active_session()
            .map(|meta| meta.title.clone()),
        Some("two".to_string()),
        "the row's press selected that session"
    );
    let index = runtime.state.active();
    assert_eq!(
        runtime.state.live().map(AppState::active),
        Some(index),
        "the app shows the session the window selected"
    );

    let one_row = session_key(&runtime, "one");
    click(&mut runtime, &one_row);
    assert_eq!(
        runtime
            .state
            .active_session()
            .map(|meta| meta.title.clone()),
        Some("one".to_string())
    );
    drop(roots);
}

#[test]
fn selecting_a_row_re_reads_the_file_panel_for_that_sessions_worktree() {
    let (data, roots, one, two) = fixture();
    std::fs::write(one.join("one_marker.rs"), "fn one() {}\n").unwrap();
    std::fs::write(two.join("two_marker.rs"), "fn two() {}\n").unwrap();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    app.new_session(&two).unwrap();
    let mut runtime = laid_out(app);

    let two_row = session_key(&runtime, "two");
    click(&mut runtime, &two_row);
    let shown = sidebar(&runtime);
    assert!(
        shown.contains("two_marker.rs") && !shown.contains("one_marker.rs"),
        "the tree is the shown session's worktree: {shown}"
    );

    let one_row = session_key(&runtime, "one");
    click(&mut runtime, &one_row);
    let shown = sidebar(&runtime);
    assert!(
        shown.contains("one_marker.rs") && !shown.contains("two_marker.rs"),
        "and a switch re-reads it rather than keeping the old rows: {shown}"
    );
    drop(roots);
}
