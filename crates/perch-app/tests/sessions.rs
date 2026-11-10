//! The session list as the window builds it: what is read from disk, what a press on a row does,
//! starting a second session on the directory already open, and what a row says when its worktree
//! has gone.
//!
//! Nothing here reaches a provider: a session is opened, listed and switched, and no turn is sent
//! to the base URL — which is why it can be a dead port.

use std::path::PathBuf;

use perch_app::{AppState, ProviderDefaults};
use perch_files::Workspace;
use perch_ui::{Item, Shell};
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
const WINDOW: (f32, f32) = (1280.0, 820.0);

fn defaults() -> ProviderDefaults {
    ProviderDefaults {
        base_url: "http://127.0.0.1:9".to_string(),
        api_key: "test-key".to_string(),
        model: "test/model".to_string(),
    }
}

/// A throwaway data directory and two worktrees named `one` and `two`, so a row's title is the
/// directory's own name rather than a random temporary one.
fn fixture() -> (TempDir, TempDir, PathBuf, PathBuf) {
    let data = tempfile::tempdir().unwrap();
    let roots = tempfile::tempdir().unwrap();
    let one = roots.path().join("one");
    let two = roots.path().join("two");
    std::fs::create_dir_all(&one).unwrap();
    std::fs::create_dir_all(&two).unwrap();
    (data, roots, one, two)
}

/// The window over `app`, laid out and drawn once, with the binary's own key map.
fn laid_out(app: AppState) -> Runtime<Shell<AppState>> {
    // The same construction the binary uses: the file panel opens on the session that is shown.
    let (metas, active) = (app.metas(), app.active());
    let shell = match metas
        .get(active)
        .and_then(|meta| Workspace::open(&meta.worktree).ok())
    {
        Some(workspace) => Shell::with_workspace(metas, active, workspace),
        None => Shell::new(metas, active),
    };
    let mut runtime = Runtime::new(
        shell.with_live(app),
        perch_ui::shell::view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.on_key =
        Some(|shell: &mut Shell<AppState>, ev, tx| perch_ui::shell::window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

/// The key of the session row whose row names `title`.
fn session_key(runtime: &Runtime<Shell<AppState>>, title: &str) -> String {
    let tree = runtime.accessibility_json();
    fn walk(node: &Value, title: &str) -> Option<String> {
        let key = node["key"].as_str().unwrap_or_default();
        let name = node["name"].as_str().unwrap_or_default();
        if key.starts_with("session-") && name.starts_with(title) {
            return Some(key.to_owned());
        }
        node["children"]
            .as_array()?
            .iter()
            .find_map(|child| walk(child, title))
    }
    walk(&tree, title).unwrap_or_else(|| panic!("no session row for {title}: {tree}"))
}

/// Every word the sidebar says.
fn sidebar(runtime: &Runtime<Shell<AppState>>) -> String {
    let tree = runtime.accessibility_json();
    find(&tree, "left-column")
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// The node keyed `key`, wherever it sits in the tree.
fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

/// A click in the middle of `key`, which is what a row's or a button's press is.
fn click(runtime: &mut Runtime<Shell<AppState>>, key: &str) {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let box_ = &node["bounds"];
    let [x, y, w, h] = [0, 1, 2, 3].map(|i| box_[i].as_f64().unwrap_or(0.0) as f32);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    // A press is dispatched by the next frame, and a real one always has time on it.
    runtime.frame(1.0 / 60.0);
}

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

#[test]
fn a_window_with_no_app_lists_nothing_and_starts_nothing() {
    let mut shell = Shell::idle(Vec::new(), 0);
    assert!(!shell.new_session(), "no app, no session");
    assert!(shell.tab_titles().is_empty(), "and nothing was invented");
}
