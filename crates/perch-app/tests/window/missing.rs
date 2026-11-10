//! The two failures the window survives: a vanished worktree and an MCP server that will not load.

use perch_app::AppState;
use perch_store::Settings;
use perch_store::settings::McpServerConfig;
use perch_ui::Item;
use perch_ui::Shell;

use crate::harness::{asked, defaults, press, said, turns};

#[test]
fn a_session_whose_worktree_vanished_refuses_the_prompt_with_a_reason() {
    let server = crate::support::serve("tests/fixtures/live_text.txt");
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let path = worktree.path().to_path_buf();
    let mut app = AppState::open_with(data.path(), defaults(&server.base_url)).unwrap();
    let index = app.new_session(&path).unwrap();
    app.switch(index).unwrap();
    // The session's worktree is gone: the turn it would run has no root.
    drop(worktree);
    let mut shell = Shell::new(app.metas(), 0).with_live(app);

    shell.set_prompt("hello");
    assert!(
        press(&mut shell, rune::Key::Enter),
        "the key is still the send key"
    );
    assert_eq!(
        shell.prompt_text(),
        "hello",
        "the text stays in the box, because there is nothing to retype"
    );
    assert!(
        asked(&shell).is_empty(),
        "a refused prompt adds no user turn: {:?}",
        asked(&shell)
    );
    let reason = said(&shell).join("\n");
    assert!(
        reason.contains("no longer exists"),
        "the column says why nothing ran: {reason:?}"
    );
    assert!(
        reason.contains(&path.display().to_string()),
        "the reason names the worktree: {reason:?}"
    );
    assert_eq!(turns(&shell), 0, "no turn ran");
}

#[test]
fn a_failed_mcp_server_is_a_note_not_a_closed_window() {
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    Settings {
        mcp_servers: vec![McpServerConfig::stdio(
            "broken",
            "/nonexistent/perch-mcp-fixture",
            Vec::new(),
        )],
        ..Settings::default()
    }
    .save(data.path())
    .unwrap();

    let mut app = AppState::open_with(data.path(), defaults("http://127.0.0.1:9"))
        .expect("a server that cannot load does not stop the app");
    let index = app.new_session(worktree.path()).unwrap();
    app.switch(index).unwrap();

    assert_eq!(app.metas().len(), 1, "the session opened anyway");
    let said: Vec<String> = app
        .active_session()
        .unwrap()
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        said.iter().any(|text| text.contains("mcp broken failed")),
        "the server that failed is said in words: {said:?}"
    );
}
