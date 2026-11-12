//! Forking a log: a copy that stops at a turn boundary, and a source that never moves.

use std::path::Path;

use perch_store::{SessionEvent, SessionLog, SessionMeta, meta_path};

/// The session every test forks, before it is written to disk.
fn source(dir: &Path) -> SessionMeta {
    SessionMeta {
        id: "earlier".to_string(),
        title: "earlier work".to_string(),
        worktree: dir.to_path_buf(),
        branch: "main".to_string(),
        model: "test/model".to_string(),
        created: 7,
    }
}

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

/// Writes [`source`] into `dir` with one turn per `(question, answer)` pair.
fn written(dir: &Path, turns: &[(&str, &str)]) -> SessionMeta {
    let meta = source(dir);
    let mut log = SessionLog::open(dir, meta.clone()).expect("the log opens");
    for (question, answer) in turns {
        for event in exchange(question, answer) {
            log.append(&event).expect("the log appends");
        }
    }
    meta
}

/// Every byte of `meta`'s log, which is what a fork must leave alone.
fn bytes(dir: &Path, meta: &SessionMeta) -> Vec<u8> {
    std::fs::read(meta_path(dir, &meta.id)).expect("the log is on disk")
}

#[test]
fn a_whole_fork_copies_every_row_under_a_new_id_and_leaves_the_source_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let from = written(dir.path(), &[("first?", "first."), ("second?", "second.")]);
    let before = bytes(dir.path(), &from);

    let fork = SessionLog::fork(dir.path(), &from, None).expect("the fork is written");

    assert_eq!(
        bytes(dir.path(), &from),
        before,
        "a fork is a copy, so the source's bytes are the same afterwards"
    );
    assert_ne!(fork.id, from.id, "a fork is a session of its own");
    assert_eq!(fork.title, "earlier work (fork)");
    assert_eq!(fork.worktree, from.worktree);
    assert_eq!(fork.branch, from.branch);
    assert_eq!(fork.model, from.model);
    assert!(
        fork.created > from.created,
        "the fork is the newer session, so the list opens on it"
    );

    let copied = SessionLog::open(dir.path(), fork)
        .unwrap()
        .events()
        .unwrap();
    let source = SessionLog::open(dir.path(), from)
        .unwrap()
        .events()
        .unwrap();
    assert_eq!(
        copied, source,
        "the whole conversation came with the fork: {copied:?}"
    );
}

#[test]
fn the_forks_header_names_the_new_id_so_opening_it_is_not_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let from = written(dir.path(), &[("first?", "first.")]);
    let fork = SessionLog::fork(dir.path(), &from, None).unwrap();

    let text = std::fs::read_to_string(meta_path(dir.path(), &fork.id)).unwrap();
    let header: SessionMeta = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(header.id, fork.id, "the first line carries the fork's id");
    assert_ne!(header.id, from.id);
    assert_eq!(header.title, "earlier work (fork)");
}

#[test]
fn a_fork_after_the_first_turn_carries_the_first_exchange_only() {
    let dir = tempfile::tempdir().unwrap();
    let from = written(dir.path(), &[("first?", "first."), ("second?", "second.")]);

    let fork = SessionLog::fork(dir.path(), &from, Some(1)).expect("the fork is written");
    let events = SessionLog::open(dir.path(), fork)
        .unwrap()
        .events()
        .unwrap();

    assert_eq!(
        events,
        exchange("first?", "first.").to_vec(),
        "the boundary is after the first `TurnEnd`"
    );
}

#[test]
fn a_boundary_inside_a_turn_is_refused_and_the_message_names_the_rule() {
    let dir = tempfile::tempdir().unwrap();
    let meta = source(dir.path());
    let mut log = SessionLog::open(dir.path(), meta.clone()).unwrap();
    for event in exchange("first?", "first.") {
        log.append(&event).unwrap();
    }
    // A second turn whose `TurnEnd` never landed: the process died mid-answer.
    for event in [
        SessionEvent::User {
            text: "second?".to_string(),
        },
        SessionEvent::Assistant {
            text: "half an ans".to_string(),
        },
    ] {
        log.append(&event).unwrap();
    }
    drop(log);

    let error = SessionLog::fork(dir.path(), &meta, Some(2))
        .expect_err("a cut inside a turn is not a fork")
        .to_string();
    assert!(
        error.contains("inside"),
        "the message names the rule it broke: {error}"
    );
    assert!(
        error.contains("TurnEnd"),
        "and the boundary it needed instead: {error}"
    );
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "a refused fork writes nothing"
    );
}

#[test]
fn a_boundary_past_the_last_finished_turn_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let from = written(dir.path(), &[("first?", "first.")]);

    let error = SessionLog::fork(dir.path(), &from, Some(2))
        .expect_err("there is no second turn")
        .to_string();
    assert!(
        error.contains("1 finished turn"),
        "the message says how many boundaries the log has: {error}"
    );
    assert!(
        error.contains("turn 2"),
        "and which one was asked for: {error}"
    );
}

#[test]
fn forking_twice_gives_each_copy_its_own_file() {
    let dir = tempfile::tempdir().unwrap();
    let from = written(dir.path(), &[("first?", "first.")]);

    let first = SessionLog::fork(dir.path(), &from, None).unwrap();
    let second = SessionLog::fork(dir.path(), &from, None).unwrap();

    assert_ne!(first.id, second.id, "two forks are two sessions");
    assert!(meta_path(dir.path(), &first.id).exists());
    assert!(meta_path(dir.path(), &second.id).exists());
}
