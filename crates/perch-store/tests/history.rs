//! The prompt history, as a caller outside the crate sees it: records merge, frecency ranks, the
//! cap holds, and two worktrees never share a file.

use std::path::Path;

use perch_store::history::{History, MAX_ENTRIES};
use perch_store::history_path::path_in;

#[test]
fn a_repeat_raises_its_count_rather_than_adding_a_row() {
    let mut history = History::default();
    history.record("fix the build", 10);
    history.record("  fix the build ", 20);
    history.record("say hi", 30);
    assert_eq!(history.len(), 2);
    assert_eq!(history.get(0), Some("fix the build"), "twice beats once");
    assert_eq!(history.entries()[0].count, 2);
}

#[test]
fn frecency_ranks_uses_above_age_and_lets_decay_undo_it() {
    const HOUR: u64 = 3_600_000;
    const DAY: u64 = 86_400_000;
    let mut history = History::default();
    for at in [0, HOUR, 2 * HOUR] {
        history.record("deploy it", at);
    }
    history.record("one small question", 10_000);
    assert_eq!(history.get(0), Some("deploy it"), "three uses beat one");
    // A month on, those uses are worth less than a single fresh one.
    let month = 40 * DAY;
    assert!(history.entries()[0].score(month) < 1.0);
    history.record("brand new", month);
    assert_eq!(history.get(0), Some("brand new"));
}

#[test]
fn the_oldest_and_rarest_fall_off_the_end() {
    let mut history = History::default();
    for index in 0..(MAX_ENTRIES + 5) {
        history.record(&format!("prompt {index}"), 0);
    }
    assert_eq!(history.len(), MAX_ENTRIES, "the cap holds");
    assert_eq!(history.get(0), Some("prompt 54"), "the newest is first");
    assert_eq!(
        history.get(MAX_ENTRIES - 1),
        Some("prompt 5"),
        "five fell off"
    );
}

#[test]
fn a_saved_history_round_trips_and_two_worktrees_never_share_one() {
    let dir = tempfile::tempdir().unwrap();
    let file = path_in(dir.path(), Path::new("/work/one"));
    assert_ne!(file, path_in(dir.path(), Path::new("/work/two")));
    let mut history = History::default();
    history.record("keep me", 100);
    history.save(&file).unwrap();
    assert_eq!(History::load(&file, 100).get(0), Some("keep me"));
    assert!(History::load(&dir.path().join("absent.json"), 0).is_empty());
}
