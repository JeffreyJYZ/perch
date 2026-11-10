//! Starting a session from the window, and a window with no app at all.

use perch_app::AppState;
use perch_ui::Shell;

use crate::harness::{click, defaults, fixture, laid_out};

#[test]
fn a_new_session_starts_on_the_directory_already_open() {
    let (data, roots, one, _two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    let mut runtime = laid_out(app);

    assert!(
        runtime.state.tab_titles().len() == 1,
        "one session is open before the press"
    );
    click(&mut runtime, "new-session");
    assert_eq!(
        runtime.state.tab_titles().len(),
        2,
        "the button opened a session without asking for a worktree"
    );
    assert_eq!(
        runtime.state.active(),
        1,
        "the new session is the one shown"
    );

    let metas = runtime
        .state
        .live()
        .map(AppState::metas)
        .unwrap_or_default();
    assert_eq!(metas.len(), 2);
    for meta in &metas {
        assert_eq!(
            meta.worktree, one,
            "every session is bound to the directory the window already had open"
        );
    }
    drop(roots);
}

#[test]
fn a_window_with_no_app_lists_nothing_and_starts_nothing() {
    let mut shell = Shell::idle(Vec::new(), 0);
    assert!(!shell.new_session(), "no app, no session");
    assert!(shell.tab_titles().is_empty(), "and nothing was invented");
}
