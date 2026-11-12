//! A history that outlives the window: per worktree, under the data directory, and capped.

use perch_store::history::MAX_ENTRIES;
use perch_ui::shell::prompt_history_file;
use rune::Key;

use crate::harness::{home, press, prompt, send, window, worktree};

#[test]
fn a_restarted_window_reads_the_prompts_an_earlier_one_sent() {
    home();
    let tree = worktree();
    let mut first = window(&tree);
    send(&mut first, "from before the restart");
    drop(first);

    let mut second = window(&tree);
    press(&mut second, Key::Up);
    assert_eq!(prompt(&second), "from before the restart");
}

#[test]
fn two_worktrees_keep_two_histories() {
    home();
    let mut one = window(&worktree());
    send(&mut one, "only in the first tree");

    let mut other = window(&worktree());
    press(&mut other, Key::Up);
    assert_eq!(prompt(&other), "", "the other worktree has sent nothing");
}

#[test]
fn the_file_is_under_the_home_the_tests_set_and_holds_at_most_the_cap() {
    home();
    let tree = worktree();
    let mut runtime = window(&tree);
    for index in 0..(MAX_ENTRIES + 5) {
        send(&mut runtime, &format!("prompt {index}"));
    }
    let path = prompt_history_file(&tree).expect("the history resolves a data directory");
    assert!(
        path.starts_with(home()),
        "never a person's own data directory: {}",
        path.display()
    );
    let text = std::fs::read_to_string(&path).unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(rows.len(), MAX_ENTRIES, "the cap holds on disk");
    assert_eq!(
        rows[0]["text"], "prompt 54",
        "the newest is first in the file"
    );
}
