//! Forking the session on screen: a second column with the same conversation, the source still
//! whole, and both listed after a relaunch.

use std::path::Path;

use perch_app::AppState;
use perch_store::{SessionEvent, SessionLog, SessionMeta, meta_path};
use perch_ui::Item;

use crate::harness::{defaults, fixture};

/// One exchange as the log writes it: the question, the reply, and the turn ending on its own.
fn exchange(question: &str, answer: &str) -> [SessionEvent; 3] {
    [
        SessionEvent::User {
            text: question.to_string(),
        },
        SessionEvent::Assistant {
            text: answer.to_string(),
        },
        SessionEvent::TurnEnd {
            reason: "done".to_string(),
            tokens_in: 5,
            tokens_out: 2,
        },
    ]
}

/// Writes the session a test forks, with one turn per `(question, answer)` pair.
///
/// The title is a name of its own rather than the folder's, so nothing here depends on the
/// directory a session was opened on.
fn write_log(dir: &Path, worktree: &Path, turns: &[(&str, &str)]) {
    let meta = SessionMeta {
        id: "earlier".to_string(),
        title: "earlier work".to_string(),
        worktree: worktree.to_path_buf(),
        branch: String::new(),
        model: "test/model".to_string(),
        created: 0,
    };
    let mut log = SessionLog::open(dir, meta).expect("the log opens");
    for (question, answer) in turns {
        for event in exchange(question, answer) {
            log.append(&event).expect("the log appends");
        }
    }
}

/// One label per row the session at `index` replays, so two columns can be read side by side.
fn rows(app: &AppState, index: usize) -> Vec<String> {
    app.sessions()[index]
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::User { text } => Some(format!("user: {text}")),
            Item::Assistant { text } => Some(format!("assistant: {text}")),
            _ => None,
        })
        .collect()
}

/// The index of the session whose log is `id`.
fn index_of(app: &AppState, id: &str) -> usize {
    app.sessions()
        .iter()
        .position(|session| session.meta.id == id)
        .unwrap_or_else(|| panic!("no session {id} among {:?}", app.metas()))
}

#[test]
fn forking_the_session_on_screen_copies_its_rows_under_a_new_id() {
    let (data, roots, one, _two) = fixture();
    write_log(
        data.path(),
        &one,
        &[("first?", "first."), ("second?", "second.")],
    );
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    assert_eq!(app.metas().len(), 1);
    let before = std::fs::read(meta_path(data.path(), "earlier")).unwrap();
    let source_rows = rows(&app, 0);

    let fork = app.fork_session(0).unwrap();

    assert_eq!(app.metas().len(), 2, "the fork is a session of its own");
    assert_eq!(app.active(), fork, "the window shows what was just made");
    assert_ne!(app.sessions()[fork].meta.id, "earlier");
    assert_eq!(app.sessions()[fork].meta.title, "earlier work (fork)");
    assert_eq!(
        rows(&app, fork),
        source_rows,
        "the copy replays the same user and assistant rows"
    );
    assert_eq!(
        rows(&app, 0),
        source_rows,
        "the source keeps its own column"
    );
    assert_eq!(
        std::fs::read(meta_path(data.path(), "earlier")).unwrap(),
        before,
        "a fork is a copy, so nothing about the source moved"
    );
    drop(roots);
}

#[test]
fn a_fork_at_the_first_turn_boundary_replays_one_user_row() {
    let (data, roots, one, _two) = fixture();
    write_log(
        data.path(),
        &one,
        &[("first?", "first."), ("second?", "second.")],
    );
    let from = {
        let app = AppState::open_with(data.path(), defaults()).unwrap();
        app.sessions()[0].meta.clone()
    };
    let fork = SessionLog::fork(data.path(), &from, Some(1)).expect("the boundary fork");

    let app = AppState::open_with(data.path(), defaults()).unwrap();
    assert_eq!(app.metas().len(), 2, "both logs are listed at start");
    assert_eq!(
        rows(&app, index_of(&app, &fork.id)),
        ["user: first?", "assistant: first."],
        "the boundary fork carries the first exchange and no more"
    );
    drop(roots);
}

#[test]
fn reopening_after_a_fork_lists_both_sessions_and_the_fork_replays_its_history() {
    let (data, roots, one, _two) = fixture();
    write_log(
        data.path(),
        &one,
        &[("first?", "first."), ("second?", "second.")],
    );
    let fork_id = {
        let mut app = AppState::open_with(data.path(), defaults()).unwrap();
        let index = app.fork_session(0).unwrap();
        app.sessions()[index].meta.id.clone()
    };

    let app = AppState::open_with(data.path(), defaults()).unwrap();
    let titles: Vec<String> = app.metas().iter().map(|meta| meta.title.clone()).collect();
    assert_eq!(titles.len(), 2, "both sessions are listed: {titles:?}");
    assert!(titles.iter().any(|title| title == "earlier work"));
    assert!(titles.iter().any(|title| title == "earlier work (fork)"));
    assert_eq!(
        rows(&app, index_of(&app, "earlier")),
        rows(&app, index_of(&app, &fork_id)),
        "the fork replays the history it was copied with"
    );

    let text = std::fs::read_to_string(meta_path(data.path(), &fork_id)).unwrap();
    let header: SessionMeta = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(header.id, fork_id, "the fork's header names the fork");
    drop(roots);
}
