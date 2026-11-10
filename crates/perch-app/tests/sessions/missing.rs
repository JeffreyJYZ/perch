//! A row whose worktree has gone: the row says missing, and the prompt is refused in words.

use perch_app::AppState;
use perch_ui::Item;
use rune::{Key, KeyEvent, Mods};

use crate::harness::{defaults, fixture, laid_out, sidebar};

#[test]
fn a_session_whose_worktree_vanished_is_named_missing_and_refuses_the_prompt() {
    let (data, roots, one, _two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    let mut runtime = laid_out(app);

    // The worktree is gone: the row says so before anything is typed.
    std::fs::remove_dir_all(&one).unwrap();
    runtime.invalidate();
    runtime.frame(0.0);
    let shown = sidebar(&runtime);
    assert!(
        shown.contains("missing"),
        "the row says the worktree is gone, in words: {shown}"
    );

    runtime.state.set_prompt("hello");
    // The key is still the send key; what it cannot do is turn into a turn with nowhere to run.
    runtime.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    runtime.invalidate();
    runtime.frame(1.0 / 60.0);
    assert_eq!(
        runtime.state.prompt_text(),
        "hello",
        "the text stays in the box, because there is nothing to retype"
    );
    assert!(
        !runtime
            .state
            .transcript()
            .items()
            .iter()
            .any(|item| matches!(item, Item::User { .. })),
        "a refused prompt adds no user turn"
    );
    let said: Vec<String> = runtime
        .state
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        said.iter().any(|text| text.contains("no longer exists")),
        "the column says why nothing ran: {said:?}"
    );
    drop(roots);
}
