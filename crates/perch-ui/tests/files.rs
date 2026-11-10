//! The file panel's state: rows in depth order, git state said in words, a numbered preview.

use std::path::Path;
use std::process::Command;

use perch_files::{PreviewText, Workspace};
use perch_ui::{FilePanel, preview_lines, status_word};
use tempfile::TempDir;

/// A throwaway repository with `files` written but nothing committed, so every one of them is
/// git's "new".
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

/// Stages everything and commits it, with identity and signing forced off so no global
/// configuration can change the outcome.
fn commit_all(repo: &TempDir, message: &str) {
    let staged = Command::new("git")
        .args(["add", "--all"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert!(
        staged.success(),
        "git add failed in {}",
        repo.path().display()
    );
    let committed = Command::new("git")
        .args([
            "-c",
            "user.email=perch-test@example.com",
            "-c",
            "user.name=perch test",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            message,
        ])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert!(
        committed.success(),
        "git commit failed in {}",
        repo.path().display()
    );
}

#[test]
fn rows_indent_by_depth_and_carry_git_state_in_words() {
    let repo = temp_repo_with(&[("src/lib.rs", "fn a() {}")]);
    let panel = FilePanel::new(&Workspace::open(repo.path()).unwrap());
    let rows = panel.rows();
    let src = rows.iter().find(|row| row.name == "src").unwrap();
    let lib = rows.iter().find(|row| row.name == "lib.rs").unwrap();

    assert_eq!(src.depth, 0);
    assert_eq!(lib.depth, 1);
    assert_eq!(status_word(lib.status), "new");
    assert_eq!(status_word(src.status), "", "a directory has no git state");
    assert_eq!(lib.icon, "code");
    assert_eq!(src.icon, "folder");
    assert!(
        lib.path.ends_with("src/lib.rs"),
        "a row knows its path: {:?}",
        lib.path
    );
}

#[test]
fn selecting_a_file_fills_the_preview_and_a_deleted_one_says_so() {
    let repo = temp_repo_with(&[("gone.rs", "x")]);
    commit_all(&repo, "add gone.rs");
    std::fs::remove_file(repo.path().join("gone.rs")).unwrap();

    let mut panel = FilePanel::new(&Workspace::open(repo.path()).unwrap());
    let row = panel
        .rows()
        .into_iter()
        .find(|row| row.name == "gone.rs")
        .expect("a deleted tracked file keeps its row");
    assert!(row.deleted);
    assert_eq!(status_word(row.status), "deleted");

    assert!(panel.select(Path::new("gone.rs")).is_err());
    assert!(panel.selected().is_none());
    let message = panel
        .message()
        .expect("the panel says why there is nothing to show");
    assert!(
        message.contains("deleted"),
        "the deletion is said in words: {message}"
    );
}

#[test]
fn selecting_a_live_file_fills_the_preview_and_arms_the_zed_handoff() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\nfn b() {}\n")]);
    let mut panel = FilePanel::new(&Workspace::open(repo.path()).unwrap());
    panel
        .select(Path::new("a.rs"))
        .expect("a live file previews");

    let preview = panel.selected().expect("the preview lives in the panel");
    assert_eq!(preview.lines.len(), 2);
    assert_eq!(preview.language.as_deref(), Some("rust"));
    assert_eq!(
        panel.open_in_zed_request(),
        Some(repo.path().canonicalize().unwrap().join("a.rs"))
    );
}

#[test]
fn preview_lines_are_one_based_and_numbered() {
    let text = PreviewText {
        lines: vec!["a".into(), "b".into()],
        language: Some("rust".into()),
    };
    assert_eq!(
        preview_lines(&text),
        vec![(1, "a".to_string()), (2, "b".to_string())]
    );
}
