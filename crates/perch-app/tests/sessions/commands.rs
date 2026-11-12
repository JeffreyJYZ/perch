//! Rename, delete and export through the real store, the window and the sidebar's own controls.

use perch_app::AppState;
use perch_store::meta_path;
use perch_ui::shell::composer_key;
use rune::{Key, KeyEvent, Mods, Tx};

use crate::harness::{click, defaults, fixture, laid_out, session_key, sidebar};

/// Presses Return in the window's composer, as a person does after typing.
fn enter(runtime: &mut rune::Runtime<perch_ui::Shell<AppState>>) {
    composer_key(
        &mut runtime.state,
        &KeyEvent {
            key: Key::Enter,
            mods: Mods::default(),
        },
        &mut Tx::default(),
    );
    // The state was mutated outside a handler, so the view is only re-run once it is marked.
    runtime.invalidate();
    runtime.frame(1.0 / 60.0);
}

#[test]
fn a_rename_changes_the_stored_title_the_sidebar_reads_and_the_next_start_keeps() {
    let (data, roots, one, _two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    app.rename_session(0, "Renamed in the store").unwrap();
    {
        let reopened = AppState::open_with(data.path(), defaults()).unwrap();
        assert_eq!(
            reopened.metas()[0].title,
            "Renamed in the store",
            "the header on disk carries the new name"
        );
    }

    let app = AppState::open_with(data.path(), defaults()).unwrap();
    let runtime = laid_out(app);
    assert!(sidebar(&runtime).contains("Renamed in the store"));
    drop(roots);
}

#[test]
fn the_rename_command_renames_the_session_the_row_names() {
    let (data, roots, one, _two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    let mut runtime = laid_out(app);
    runtime.state.set_prompt("/rename From the composer");
    enter(&mut runtime);
    assert!(
        sidebar(&runtime).contains("From the composer"),
        "{}",
        sidebar(&runtime)
    );
    let reopened = AppState::open_with(data.path(), defaults()).unwrap();
    assert_eq!(reopened.metas()[0].title, "From the composer");
    drop(roots);
}

#[test]
fn deleting_a_session_removes_its_row_and_its_log() {
    let (data, roots, one, _two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    let id = app.metas()[0].id.clone();
    let path = meta_path(data.path(), &id);
    assert!(path.exists(), "the log is written when the session opens");

    app.delete_session(0).unwrap();
    assert!(app.sessions().is_empty(), "the runtime was dropped");
    assert!(!path.exists(), "the log was deleted with it");
    drop(roots);
}

#[test]
fn the_sidebar_delete_control_arms_then_removes_and_the_window_follows() {
    let (data, roots, one, _two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    let id = app.metas()[0].id.clone();
    let mut runtime = laid_out(app);
    let row = session_key(&runtime, "one");
    assert!(row.starts_with("session-"));

    click(&mut runtime, "session-delete-0");
    assert_eq!(
        runtime.state.session_count(),
        1,
        "one press only arms the delete"
    );
    click(&mut runtime, "session-delete-0");
    assert_eq!(runtime.state.session_count(), 0, "the second removes it");
    assert!(
        runtime
            .state
            .live()
            .is_some_and(|app| app.sessions().is_empty()),
        "the app dropped the runtime"
    );
    assert!(!meta_path(data.path(), &id).exists(), "and its log");
    drop(roots);
}

#[test]
fn export_from_the_window_writes_the_transcript_under_the_data_directory() {
    let (data, roots, one, _two) = fixture();
    let mut app = AppState::open_with(data.path(), defaults()).unwrap();
    app.new_session(&one).unwrap();
    app.active_session_mut()
        .unwrap()
        .transcript
        .push_user("exported question");
    let mut runtime = laid_out(app);
    runtime.state.set_prompt("/export");
    enter(&mut runtime);

    let found = std::fs::read_dir(data.path())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .expect("a file was exported");
    let text = std::fs::read_to_string(found).unwrap();
    assert_eq!(text.lines().next(), Some("you: exported question"));
    drop(roots);
}
