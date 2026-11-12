//! The sidebar's filter: typing narrows the rows by title, worktree and branch, the count says
//! how many of how many, and Escape puts every row back with the selection where it was.

use perch_store::SessionMeta;

use crate::harness::{click, node, press, session, type_text, window, words};
use rune::Key;

/// Twelve sessions, three of whose titles hold `alpha`.
fn twelve() -> Vec<SessionMeta> {
    (0..12)
        .map(|index| match index {
            0 => session("Alpha one", "/work/one", "dev"),
            3 => session("Alpha two", "/work/two", "dev"),
            7 => session("Alpha three", "/work/three", "dev"),
            _ => session(
                &format!("Session {index}"),
                &format!("/work/{index}"),
                "dev",
            ),
        })
        .collect()
}

#[test]
fn typing_narrows_by_title_worktree_and_branch_whatever_the_case() {
    let mut runtime = window(
        vec![
            session("Fix the parser", "/work/alpha-tools", "feature/login"),
            session("Unrelated", "/work/beta", "dev"),
        ],
        0,
    );
    click(&mut runtime, "session-filter");

    type_text(&mut runtime, "parser");
    assert_eq!(
        runtime.state.session_rows().len(),
        1,
        "the title narrowed the list"
    );
    assert!(
        words(&runtime).contains("1 of 2 sessions"),
        "the count says how many of how many: {}",
        words(&runtime)
    );

    press(&mut runtime, Key::Escape);
    type_text(&mut runtime, "ALPHA");
    assert_eq!(
        runtime.state.session_rows().len(),
        1,
        "the worktree path matched, case-insensitively"
    );

    press(&mut runtime, Key::Escape);
    type_text(&mut runtime, "login");
    assert_eq!(runtime.state.session_rows().len(), 1, "the branch matched");
}

#[test]
fn the_count_line_says_three_of_twelve_sessions() {
    let mut runtime = window(twelve(), 0);
    click(&mut runtime, "session-filter");
    type_text(&mut runtime, "alpha");

    assert_eq!(runtime.state.session_rows().len(), 3);
    assert_eq!(runtime.state.session_count(), 12, "no session was closed");
    assert!(
        words(&runtime).contains("3 of 12 sessions"),
        "the count line says it: {}",
        words(&runtime)
    );
}

#[test]
fn escape_clears_the_filter_and_keeps_the_selection() {
    let mut runtime = window(
        vec![
            session("Alpha one", "/work/one", "dev"),
            session("Beta two", "/work/two", "dev"),
            session("Gamma three", "/work/three", "dev"),
        ],
        1,
    );
    click(&mut runtime, "session-filter");
    type_text(&mut runtime, "alpha");
    assert_eq!(
        runtime.state.session_rows().len(),
        1,
        "the active session is filtered out from under the selection"
    );

    press(&mut runtime, Key::Escape);
    assert_eq!(runtime.state.session_rows().len(), 3, "every row is back");
    assert_eq!(
        runtime.state.active(),
        1,
        "the selection stayed on the session it was on"
    );
    let tree = runtime.accessibility_json();
    assert!(
        node(&tree, "session-1").is_some(),
        "the filtered-out row is drawn again"
    );
}

#[test]
fn a_filter_that_matches_nothing_says_so_in_the_list() {
    let mut runtime = window(twelve(), 0);
    click(&mut runtime, "session-filter");
    type_text(&mut runtime, "zzzz");

    assert!(runtime.state.session_rows().is_empty());
    assert!(
        words(&runtime).contains("no session matches"),
        "a miss is said in words in the sidebar: {}",
        words(&runtime)
    );
    assert_eq!(runtime.state.session_count(), 12, "no session was closed");
}
