//! A throwaway repository, and the two ways a test reads the tree built over it.

use std::process::Command;

use perch_files::Node;
use tempfile::TempDir;

/// A throwaway repository, never the perch checkout.
pub fn temp_repo() -> TempDir {
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
    dir
}

/// Stages everything and commits it, with identity and signing forced off so no global
/// configuration can change the outcome.
pub fn commit_all(repo: &TempDir, message: &str) {
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

pub fn walk(node: &Node) -> Vec<&Node> {
    let mut seen = vec![node];
    for child in &node.children {
        seen.extend(walk(child));
    }
    seen
}

pub fn flatten(nodes: &[Node]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(walk)
        .map(|n| n.name.clone())
        .collect()
}
