//! The saved shell command end to end: a new tab runs it, and a tab already open keeps its own.

use std::sync::Arc;

use perch_app::{AppState, ProviderDefaults};
use perch_store::Settings;
use perch_store::secret::MemorySecrets;
use perch_ui::Shell;

use crate::harness::{pump_until, type_line, use_sh};

/// The app over `data`, with its key in memory rather than the login keychain.
fn open(data: &tempfile::TempDir) -> AppState {
    AppState::open_with_secrets(
        data.path(),
        ProviderDefaults::default(),
        Box::new(Arc::new(MemorySecrets::new())),
    )
    .expect("a data directory with no sessions opens")
}

/// A window over an app with one session on `worktree`, and the draft the panel saves.
#[allow(clippy::type_complexity)]
fn window(
    data: &tempfile::TempDir,
    worktree: &std::path::Path,
) -> (Shell<AppState>, perch_ui::SettingsDraft) {
    let mut app = open(data);
    let index = app.new_session(worktree).expect("a session opens");
    let shell = Shell::new(app.metas(), index).with_live(app);
    let draft = perch_ui::SettingsDraft {
        base_url: "https://example.com/v1".to_string(),
        model: "test/model".to_string(),
        theme: "dark".to_string(),
        ..Default::default()
    };
    (shell, draft)
}

/// Saves `command` through the panel, as a person would, and reads it back.
fn save_shell(shell: &mut Shell<AppState>, draft: &perch_ui::SettingsDraft, command: &str) {
    let mut draft = draft.clone();
    draft.shell = command.to_string();
    shell.toggle_settings();
    shell.stage_settings(draft);
    assert!(shell.save_settings(), "the panel saved `{command}`");
    shell.close_settings();
    assert_eq!(
        shell.live().map(|app| app.settings().shell),
        Some(command.to_string()),
        "the pane reads back the line that was saved"
    );
}

#[test]
fn a_new_tab_runs_the_saved_command() {
    use_sh();
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let (mut shell, draft) = window(&data, worktree.path());

    // A whole line, quoting and all: the marker is printed by the command the line named.
    save_shell(
        &mut shell,
        &draft,
        "/bin/sh -c 'echo perch-shell-line-42; sleep 30'",
    );
    let index = shell.open_terminal().expect("the saved line starts a tab");
    assert_eq!(index, 0);
    assert!(
        pump_until(&mut shell, |shell| {
            shell
                .active_terminal()
                .and_then(|tab| tab.frame())
                .is_some_and(|frame| {
                    perch_ui::paint_rows(frame)
                        .iter()
                        .any(|run| run.text.contains("perch-shell-line-42"))
                })
        }),
        "the tab runs the line settings holds"
    );
}

#[test]
fn a_save_leaves_a_running_tabs_shell_alone() {
    use_sh();
    let data = tempfile::tempdir().unwrap();
    let worktree = tempfile::tempdir().unwrap();
    let (mut shell, draft) = window(&data, worktree.path());

    save_shell(&mut shell, &draft, "/bin/sh");
    shell
        .open_terminal()
        .expect("a shell starts in the worktree");
    type_line(&mut shell, "pwd");
    assert!(
        pump_until(&mut shell, |shell| {
            shell
                .active_terminal()
                .is_some_and(|tab| tab.frame().is_some())
        }),
        "the first tab is a running shell"
    );

    // A save that names another shell: the tab already open is not restarted under the person.
    save_shell(&mut shell, &draft, "/bin/cat");
    assert_eq!(
        Settings::load(data.path()).unwrap().shell,
        "/bin/cat",
        "the file holds the new line"
    );

    // `exit` ends `sh` and is only echoed by `cat`, so a tab that still ends was never restarted
    // onto the line the save wrote.
    type_line(&mut shell, "exit");
    assert!(
        pump_until(&mut shell, |shell| {
            shell
                .active_terminal()
                .is_some_and(|tab| tab.ended().is_some())
        }),
        "the running tab is still the shell it started, so `exit` ended it"
    );
    assert_eq!(
        shell.terminals().len(),
        1,
        "the save did not open, close or replace a tab"
    );
}
