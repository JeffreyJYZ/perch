//! The shell's acceptance tests, all through the accessibility tree: a tab switch moves the
//! active session, the window names every region the layout promises, and the regions the
//! composed window reads from — a worktree, a transcript and a terminal frame — reach the tree
//! they are drawn into and nowhere else.

use std::path::PathBuf;
use std::process::Command;

use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};
use perch_files::Workspace;
use perch_store::SessionMeta;
use perch_term::replay;
use perch_ui::Shell;
use rune::{Env, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// The window the tests lay out, in points: the same size the binary opens.
const WINDOW: (f32, f32) = (1280.0, 820.0);

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

/// A throwaway repository holding `files`, so the file panel reads a real worktree.
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
        let path = dir.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }
    dir
}

/// Lays `shell` out at [`WINDOW`] and returns its accessibility tree.
fn tree_of(shell: Shell) -> Value {
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime.accessibility_json()
}

/// The node the app keyed `key`, wherever it sits.
fn find_key<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node.get("key").and_then(Value::as_str) == Some(key) {
        return Some(node);
    }
    let children = node.get("children").and_then(Value::as_array)?;
    children.iter().find_map(|child| find_key(child, key))
}

/// Every word `node`'s subtree says out loud: names, values and placeholders.
fn words(node: &Value, out: &mut Vec<String>) {
    for field in ["name", "value", "placeholder", "description"] {
        if let Some(text) = node.get(field).and_then(Value::as_str) {
            out.push(text.to_owned());
        }
    }
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        words(child, out);
    }
}

/// Whether `node`'s subtree says `needle` anywhere.
fn says(node: &Value, needle: &str) -> bool {
    let mut out = Vec::new();
    words(node, &mut out);
    out.iter().any(|text| text.contains(needle))
}

/// The drawn box of `node`: `[x, y, width, height]` in window points.
fn bounds(node: &Value) -> [f64; 4] {
    let box_ = node
        .get("bounds")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("the node has no bounds: {node}"));
    [0, 1, 2, 3].map(|i| box_[i].as_f64().expect("a bound is a number"))
}

#[test]
fn selecting_a_tab_changes_the_active_session() {
    let mut shell = Shell::new(vec![meta("a"), meta("b")], 0);
    assert_eq!(shell.active(), 0);
    shell.select(1);
    assert_eq!(shell.active(), 1);
    assert_eq!(shell.tab_titles(), vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn tree_dump_names_every_region() {
    let out = Command::new(env!("CARGO_BIN_EXE_perch"))
        .args(["--tree", "--offline"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for region in ["sessions", "transcript", "terminal", "files"] {
        assert!(
            text.contains(region),
            "accessibility tree is missing {region}:\n{text}"
        );
    }
}

#[test]
fn a_file_row_from_the_workspace_reaches_the_file_panel() {
    let repo = temp_repo_with(&[("alpha_widget.rs", "fn alpha() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let tree = tree_of(Shell::with_workspace(vec![meta("work")], 0, workspace));

    let files = find_key(&tree, "files").expect("the file panel is in the tree");
    assert!(
        says(files, "alpha_widget.rs"),
        "the file row names the file it was built from: {files}"
    );
}

#[test]
fn the_transcript_shows_text_and_a_tool_cards_status_word() {
    let mut shell = Shell::new(vec![meta("work")], 0);
    {
        let transcript = shell.transcript_mut();
        transcript.push_user("why is the build red?");
        transcript.apply(&AgentEvent::TextDelta("Two jobs failed.".into()));
        transcript.apply(&AgentEvent::ToolCall(ToolCall {
            id: "call-1".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({ "path": "src/main.rs" }),
        }));
        transcript.apply(&AgentEvent::ToolResult {
            id: "call-1".to_string(),
            ok: true,
            text: "fn main() {}\n".to_string(),
        });
        transcript.apply(&AgentEvent::TurnEnd {
            reason: EndReason::Done,
            usage: Usage::default(),
        });
    }

    let tree = tree_of(shell);
    let transcript = find_key(&tree, "transcript").expect("the transcript region is in the tree");
    assert!(
        says(transcript, "Two jobs failed."),
        "the assistant's text is drawn: {transcript}"
    );
    assert!(
        says(transcript, "why is the build red?"),
        "the user's turn is drawn: {transcript}"
    );
    assert!(
        says(transcript, "done"),
        "the tool card says how it ended: {transcript}"
    );
}

#[test]
fn terminal_rows_paint_in_the_dock_and_never_in_the_sidebar() {
    let frame_text = "perch-dock-marker-3141";
    let frame = replay(format!("{frame_text}\r\n$ ").as_bytes(), 40, 3);
    let tree = tree_of(Shell::new(vec![meta("work")], 0).with_frame("zsh", frame));

    let dock = find_key(&tree, "terminal").expect("the terminal dock is in the tree");
    assert!(
        says(dock, frame_text),
        "the dock paints the seeded frame's row: {dock}"
    );

    let sidebar = find_key(&tree, "left-column").expect("the sidebar is in the tree");
    assert!(
        !says(sidebar, frame_text),
        "terminal text belongs to the dock alone: {sidebar}"
    );
}

#[test]
fn each_terminal_tab_paints_only_its_own_rows() {
    let seeded = || {
        Shell::new(vec![meta("work")], 0)
            .with_frame("first", replay(b"first-tab-marker", 40, 3))
            .with_frame("second", replay(b"second-tab-marker", 40, 3))
    };

    // A seeded tab is the newest, and the newest is the one the dock shows.
    let latest = tree_of(seeded());
    let dock = find_key(&latest, "terminal").expect("the dock is in the tree");
    assert!(
        find_key(dock, "terminal-tab-row").is_some(),
        "two tabs get a strip that names them: {dock}"
    );
    assert!(
        says(dock, "second-tab-marker"),
        "the dock paints the newest tab's frame: {dock}"
    );
    assert!(
        !says(dock, "first-tab-marker"),
        "the tab that is not shown keeps its rows out of the dock: {dock}"
    );

    // Selecting the other tab swaps which frame the dock paints.
    let mut shell = seeded();
    shell.select_terminal(0);
    let chosen = tree_of(shell);
    let dock = find_key(&chosen, "terminal").expect("the dock is in the tree");
    assert!(
        says(dock, "first-tab-marker"),
        "the selected tab's frame is painted: {dock}"
    );
    assert!(
        !says(dock, "second-tab-marker"),
        "the unselected tab's rows are gone: {dock}"
    );
}

#[test]
fn the_dock_sits_under_the_transcript_and_the_sidebar_runs_full_height() {
    let frame = replay(b"$ echo hi", 40, 3);
    let tree = tree_of(Shell::new(vec![meta("work")], 0).with_frame("zsh", frame));

    let sidebar = bounds(find_key(&tree, "left-column").expect("the sidebar is in the tree"));
    let transcript = bounds(find_key(&tree, "transcript").expect("the transcript is in the tree"));
    let dock = bounds(find_key(&tree, "terminal").expect("the dock is in the tree"));
    let (window_w, window_h) = WINDOW;

    assert_eq!(sidebar[0], 0.0, "the sidebar starts at the window's left");
    assert_eq!(sidebar[1], 0.0, "the sidebar starts at the window's top");
    assert_eq!(
        sidebar[3],
        f64::from(window_h),
        "the sidebar runs the full window height"
    );

    assert!(
        dock[0] >= f64::from(perch_ui::COLUMN),
        "the dock starts right of the left column, at {}",
        dock[0]
    );
    assert!(
        dock[0] >= sidebar[0] + sidebar[2],
        "the dock does not overlap the sidebar"
    );
    assert!(
        transcript[1] < dock[1],
        "the dock is below the transcript ({} < {})",
        transcript[1],
        dock[1]
    );
    assert!(
        transcript[1] + transcript[3] <= dock[1] + 0.5,
        "the transcript ends where the dock starts"
    );
    assert!(
        dock[1] + dock[3] <= f64::from(window_h) + 0.5,
        "the dock stays inside the window"
    );
    assert_eq!(
        dock[2], transcript[2],
        "the dock is no wider than the transcript"
    );
    assert!(
        dock[0] + dock[2] <= f64::from(window_w) + 0.5,
        "the dock stays inside the window's width"
    );
}
