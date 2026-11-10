//! The window's wiring, driven through a laid-out runtime: a file row's press fills the preview
//! and the preview's button hands the path to the app's editor, and Enter on a focused tool card
//! opens it — while Enter with nothing focused opens nothing.
//!
//! The editor is a fake, so no test starts a command; the app's own `Zed` is the only place an
//! `open -a` can run.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use perch_agent::{AgentEvent, ToolCall};
use perch_files::Workspace;
use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::files::Editor;
use perch_ui::shell::window_key;
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// An editor that records what it was asked to open, and never runs anything.
#[derive(Default)]
struct Fake {
    opened: Mutex<Vec<PathBuf>>,
    refusal: Option<String>,
}

impl Fake {
    fn opened(&self) -> Vec<PathBuf> {
        self.opened.lock().unwrap().clone()
    }
}

impl Editor for Fake {
    fn open_in_zed(&self, path: &Path) -> Result<(), String> {
        self.opened.lock().unwrap().push(path.to_path_buf());
        match &self.refusal {
            Some(words) => Err(words.clone()),
            None => Ok(()),
        }
    }
}

fn meta(title: &str) -> SessionMeta {
    SessionMeta {
        id: title.to_string(),
        title: title.to_string(),
        worktree: PathBuf::from("/work"),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    }
}

/// A throwaway repository holding `files`, so the panel reads a real worktree.
fn temp_repo_with(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "git init failed in {}",
        dir.path().display()
    );
    for (path, contents) in files {
        std::fs::write(dir.path().join(path), contents).unwrap();
    }
    dir
}

/// Stages everything and commits it, with identity and signing forced off so no global
/// configuration can change the outcome.
fn commit_all(repo: &TempDir) {
    for args in [
        ["add", "--all"].as_slice(),
        [
            "-c",
            "user.email=perch-test@example.com",
            "-c",
            "user.name=perch test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "add",
        ]
        .as_slice(),
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }
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

/// The drawn box of `key`: `[x, y, width, height]` in window points.
fn bounds(runtime: &Runtime<Shell>, key: &str) -> [f32; 4] {
    let tree = runtime.accessibility_json();
    let node = find(&tree, key).unwrap_or_else(|| panic!("no {key} in {tree}"));
    let box_ = &node["bounds"];
    [
        number(&box_[0]),
        number(&box_[1]),
        number(&box_[2]),
        number(&box_[3]),
    ]
}

fn number(value: &Value) -> f32 {
    value.as_f64().unwrap_or(0.0) as f32
}

/// A click in the middle of `key`, which is what focuses a control or presses a row.
fn click(runtime: &mut Runtime<Shell>, key: &str) {
    let [x, y, w, h] = bounds(runtime, key);
    runtime.pointer_down(x + w / 2.0, y + h / 2.0);
    runtime.pointer_up(x + w / 2.0, y + h / 2.0);
    // A press is dispatched by the next frame, and a real one always has time on it.
    runtime.frame(1.0 / 60.0);
}

/// Everything the tree says, so a test can assert on a word the window draws.
fn words(runtime: &Runtime<Shell>) -> String {
    runtime.accessibility_json().to_string()
}

/// A laid-out window whose keys go through the binary's own map.
fn window(shell: Shell) -> Runtime<Shell> {
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.on_key = Some(|shell: &mut Shell, ev, tx| window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

fn enter() -> KeyEvent {
    KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    }
}

#[test]
fn a_row_press_fills_the_preview_and_its_button_hands_the_path_to_the_editor() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\nfn b() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake::default());
    let shell = Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor.clone());
    let mut runtime = window(shell);

    // The row names the file, and a press on it is what selects it.
    click(&mut runtime, "file-row-0");
    let selected = runtime
        .state
        .files()
        .and_then(perch_ui::FilePanel::selected_path)
        .map(Path::to_path_buf);
    assert_eq!(
        selected.as_deref(),
        Some(repo.path().canonicalize().unwrap().join("a.rs").as_path()),
        "the row's press selected the file it names: {}",
        words(&runtime)
    );

    // The preview is beside the tree, with the file's lines and the one action.
    let shown = words(&runtime);
    assert!(shown.contains("fn a() {}"), "the preview draws the file");
    assert!(shown.contains("Open in Zed"), "the action is offered");

    click(&mut runtime, "open-in-zed");
    assert_eq!(
        editor.opened(),
        [repo.path().canonicalize().unwrap().join("a.rs")],
        "the button handed the selected path to the app's editor, and nothing else"
    );
}

#[test]
fn the_button_does_nothing_until_a_file_is_selected() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake::default());
    let runtime =
        window(Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor.clone()));
    let tree = runtime.accessibility_json();
    assert!(
        find(&tree, "preview").is_none(),
        "with nothing selected the column is the tree alone: {tree}"
    );
    assert!(
        find(&tree, "open-in-zed").is_none(),
        "there is no action to press into nothing: {tree}"
    );
    assert!(editor.opened().is_empty(), "nothing was handed over");
}

#[test]
fn enter_opens_the_focused_tool_card_and_opens_nothing_with_no_card_focused() {
    let mut shell = Shell::idle(vec![meta("work")], 0);
    {
        let transcript = shell.transcript_mut();
        transcript.apply(&AgentEvent::ToolCall(ToolCall {
            id: "call-1".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({ "path": "src/main.rs" }),
        }));
        transcript.apply(&AgentEvent::ToolResult {
            id: "call-1".to_string(),
            ok: true,
            text: (1..=12).map(|n| format!("line {n}\n")).collect(),
        });
    }
    let mut runtime = window(shell);
    assert!(
        words(&runtime).contains("showing first 5"),
        "a card starts collapsed: {}",
        words(&runtime)
    );

    // Nothing is focused, so Enter is the composer's: it opens no card.
    runtime.key(enter());
    runtime.frame(0.0);
    assert!(
        !runtime.state.transcript().is_expanded("call-1"),
        "Enter with no card focused opens nothing"
    );

    // A press on the card focuses it, and then Enter is the card's.
    click(&mut runtime, "tool-0");
    runtime.key(enter());
    runtime.frame(0.0);
    assert!(
        runtime.state.transcript().is_expanded("call-1"),
        "Enter on the focused card opens it: {}",
        words(&runtime)
    );
    let opened = words(&runtime);
    assert!(
        opened.contains("line 12") && !opened.contains("showing first 5"),
        "the whole output replaces the cap: {opened}"
    );

    runtime.key(enter());
    runtime.frame(0.0);
    assert!(
        !runtime.state.transcript().is_expanded("call-1"),
        "a second Enter closes it again"
    );

    // Space is Rune's own activation key for a focused control, and a card has no press handler;
    // the root's key handler runs first, so Space reaches the same toggle the window map does.
    runtime.key(KeyEvent {
        key: Key::Text(" ".to_string()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
    assert!(
        runtime.state.transcript().is_expanded("call-1"),
        "Space on the focused card opens it too: {}",
        words(&runtime)
    );
}

#[test]
fn a_deleted_row_says_in_words_why_it_cannot_be_previewed() {
    let repo = temp_repo_with(&[("gone.rs", "x")]);
    commit_all(&repo);
    std::fs::remove_file(repo.path().join("gone.rs")).unwrap();
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake::default());
    let mut runtime =
        window(Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor.clone()));

    // The row is still there, and the tree already says what git reports.
    assert!(
        words(&runtime).contains("gone.rs, deleted"),
        "a tracked file git reports deleted keeps its row: {}",
        words(&runtime)
    );

    click(&mut runtime, "file-row-0");
    let tree = runtime.accessibility_json();
    let preview = find(&tree, "preview").expect("the reason is drawn where a preview would be");
    assert!(
        preview.to_string().contains("deleted"),
        "the panel says the deletion in words instead of an empty page: {preview}"
    );
    assert!(
        editor.opened().is_empty(),
        "nothing that is not there is handed to the editor"
    );
}

#[test]
fn a_refused_handoff_is_said_in_words_beside_the_preview() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let editor = Arc::new(Fake {
        refusal: Some("Zed is not installed".to_string()),
        ..Fake::default()
    });
    let mut runtime =
        window(Shell::with_workspace(vec![meta("work")], 0, workspace).with_editor(editor));

    click(&mut runtime, "file-row-0");
    click(&mut runtime, "open-in-zed");
    let shown = words(&runtime);
    assert!(
        shown.contains("Zed did not open") && shown.contains("Zed is not installed"),
        "the refusal is drawn where the file is: {shown}"
    );
    assert!(
        shown.contains("fn a() {}"),
        "a refusal keeps the preview on screen: {shown}"
    );
}
