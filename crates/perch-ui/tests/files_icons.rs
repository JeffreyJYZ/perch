//! The tree as a window draws it: one kind icon on every row, and a folder's own drawing on the
//! folder's row.

use std::path::Path;
use std::process::Command;

use perch_files::{Kind, Workspace};
use perch_ui::Shell;
use perch_ui::shell::Pane;
use rune::{Env, Runtime};
use serde_json::Value;
use tempfile::TempDir;

/// A laid-out window, at the size the app opens at.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A throwaway repository with `files` written but nothing committed.
fn temp_repo_with(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success(), "git init failed");
    for (path, contents) in files {
        let path = dir.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }
    dir
}

/// Every node of `node`, depth first.
fn nodes(node: &Value, out: &mut Vec<&Value>) {
    out.push(node);
    for child in node["children"].as_array().into_iter().flatten() {
        nodes(child, out);
    }
}

#[test]
fn every_row_carries_one_kind_icon_and_a_folder_carries_its_own() {
    let repo = temp_repo_with(&[
        ("src/main.rs", "fn main() {}"),
        ("notes.md", "# notes"),
        ("plain.txt", "words"),
    ]);
    let workspace = Workspace::open(repo.path()).unwrap();
    let shell: Shell = Shell::with_workspace(Vec::new(), 0, workspace);
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.frame(0.0);
    let tree = runtime.accessibility_json();
    let mut all = Vec::new();
    nodes(&tree, &mut all);

    let rows = runtime.state.files().map(|panel| panel.rows()).unwrap();
    assert_eq!(rows.len(), 4, "src, main.rs, notes.md, plain.txt");
    for (index, row) in rows.iter().enumerate() {
        let key = format!("file-kind-{index}");
        let drawn: Vec<&&Value> = all
            .iter()
            .filter(|node| node["key"].as_str() == Some(key.as_str()))
            .collect();
        assert_eq!(drawn.len(), 1, "{} carries exactly one kind icon", row.name);
        let rect = &drawn[0]["bounds"];
        assert!(
            rect[2].as_f64().unwrap_or(0.0) >= 16.0 && rect[3].as_f64().unwrap_or(0.0) >= 16.0,
            "{} is drawn at its full 16 points",
            row.name
        );
    }
    let folder = rows.iter().find(|row| row.name == "src").unwrap();
    let rust = rows.iter().find(|row| row.name == "main.rs").unwrap();
    assert_eq!(folder.kind, Kind::Folder);
    assert_eq!(rust.kind, Kind::Rust);
    assert_ne!(folder.kind, rust.kind, "a folder is not drawn as a file");
    assert!(
        rows.iter()
            .any(|row| row.name == "notes.md" && row.kind == Kind::Markdown),
        "the markdown file keeps its kind"
    );
}

#[test]
fn a_file_row_opens_its_pane_from_the_press_the_icon_sits_in() {
    let repo = temp_repo_with(&[("src/main.rs", "fn main() {}")]);
    let workspace = Workspace::open(repo.path()).unwrap();
    let mut shell: Shell = Shell::with_workspace(Vec::new(), 0, workspace);
    assert!(shell.open_file(Path::new("src/main.rs")));
    assert_eq!(
        shell.pane(),
        &Pane::File(repo.path().canonicalize().unwrap().join("src/main.rs")),
        "the row's press still opens the file"
    );
}
