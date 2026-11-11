//! One read of the worktree for the finder: every file it can open.
//!
//! The walk is the file panel's own: [`Workspace::tree`] honours `.gitignore`, `.git/info/exclude`
//! and the hidden rule, so a checkout's ignored and dot paths are already absent. Three names are
//! skipped whatever a `.gitignore` says, because a finder that offers a build artifact is worse
//! than one that does not: `.git`, `target` and `node_modules`.
//!
//! This runs when the overlay opens, never from a frame and never per keystroke.

use std::path::{Path, PathBuf};

use perch_files::{Node, NodeKind, Workspace};

use super::finder::MAX_FILES;

/// The directories the finder never offers, at any depth.
const SKIP: [&str; 3] = [".git", "target", "node_modules"];

/// Every file `workspace` holds, relative to its root, and whether the list stopped at
/// [`MAX_FILES`].
pub(super) fn scan(workspace: &Workspace) -> (Vec<PathBuf>, bool) {
    let mut files = Vec::new();
    let mut truncated = false;
    collect(
        &workspace.tree(),
        workspace.root(),
        &mut files,
        &mut truncated,
    );
    (files, truncated)
}

/// Walks `nodes` in tree order, keeping a file's path relative to `root`.
fn collect(nodes: &[Node], root: &Path, files: &mut Vec<PathBuf>, truncated: &mut bool) {
    for node in nodes {
        match node.kind {
            NodeKind::Dir if skipped(&node.name) => {}
            NodeKind::Dir => collect(&node.children, root, files, truncated),
            NodeKind::File => {
                if files.len() >= MAX_FILES {
                    *truncated = true;
                    return;
                }
                let relative = node.path.strip_prefix(root).unwrap_or(&node.path);
                files.push(relative.to_path_buf());
            }
        }
        if *truncated {
            return;
        }
    }
}

/// Whether a directory of this name is one the finder skips.
fn skipped(name: &str) -> bool {
    SKIP.contains(&name)
}
