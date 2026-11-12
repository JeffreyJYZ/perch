//! Up and Down over an empty composer: the walk back through what this worktree has sent.
//!
//! The presses are real ones through the window's own key map, laid out by Rune, so a recalled
//! prompt is only recalled if the field really withheld the key.

use crate::harness;

use rune::Key;

use crate::harness::{home, press, prompt, send, type_text, window, worktree};

#[test]
fn up_walks_back_and_down_returns_to_the_draft() {
    home();
    let mut runtime = window(&worktree());
    send(&mut runtime, "first prompt");
    send(&mut runtime, "second prompt");
    assert_eq!(prompt(&runtime), "", "sending empties the box");

    press(&mut runtime, Key::Up);
    assert_eq!(prompt(&runtime), "second prompt", "the last one sent");
    press(&mut runtime, Key::Up);
    assert_eq!(prompt(&runtime), "first prompt");
    press(&mut runtime, Key::Up);
    assert_eq!(prompt(&runtime), "first prompt", "nothing further back");

    press(&mut runtime, Key::Down);
    assert_eq!(prompt(&runtime), "second prompt");
    press(&mut runtime, Key::Down);
    assert_eq!(prompt(&runtime), "", "past the newest is the draft again");
    press(&mut runtime, Key::Down);
    assert_eq!(prompt(&runtime), "", "and no further");
}

#[test]
fn up_with_text_in_the_box_leaves_the_text_alone() {
    home();
    let mut runtime = window(&worktree());
    send(&mut runtime, "an earlier prompt");
    type_text(&mut runtime, "half a thought");
    press(&mut runtime, Key::Up);
    assert_eq!(
        prompt(&runtime),
        "half a thought",
        "Up over text a person typed is the field's"
    );
}

#[test]
fn an_edit_of_a_recalled_prompt_ends_the_walk() {
    home();
    let mut runtime = window(&worktree());
    send(&mut runtime, "one");
    send(&mut runtime, "two");
    press(&mut runtime, Key::Up);
    assert_eq!(prompt(&runtime), "two");
    type_text(&mut runtime, "!");
    assert_eq!(prompt(&runtime), "two!");
    press(&mut runtime, Key::Up);
    assert_eq!(
        prompt(&runtime),
        "two!",
        "the walk ended, so Up is the field's"
    );
}

#[test]
fn a_recalled_prompt_is_editable_and_never_sent_by_the_walk() {
    home();
    let mut runtime = window(&worktree());
    send(&mut runtime, "make it red");
    press(&mut runtime, Key::Up);
    type_text(&mut runtime, " please");
    assert_eq!(prompt(&runtime), "make it red please");
    assert_eq!(
        harness::sent(&runtime),
        vec!["make it red"],
        "walking back and editing sent nothing"
    );
    press(&mut runtime, Key::Enter);
    assert_eq!(
        harness::sent(&runtime),
        vec!["make it red", "make it red please"]
    );
}

#[test]
fn a_repeat_ranks_above_a_prompt_sent_once() {
    home();
    let mut runtime = window(&worktree());
    send(&mut runtime, "repeat me");
    send(&mut runtime, "repeat me");
    send(&mut runtime, "once");
    press(&mut runtime, Key::Up);
    assert_eq!(
        prompt(&runtime),
        "repeat me",
        "two uses outrank one, even though the one-off is newer"
    );
    press(&mut runtime, Key::Up);
    assert_eq!(prompt(&runtime), "once", "and the repeat is only one row");
    press(&mut runtime, Key::Up);
    assert_eq!(
        prompt(&runtime),
        "once",
        "past the oldest, the oldest stays"
    );
}
