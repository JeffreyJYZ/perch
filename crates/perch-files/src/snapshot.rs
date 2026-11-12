//! A worktree's files at a moment, kept as a git tree object.
//!
//! A turn's starting point has to be capturable without touching anything a person owns: not
//! their index, not `HEAD`, not their stash, not the working tree. A tree object is built in a
//! scratch index file under the repository's own git directory, so the person's index is never
//! opened, and `git write-tree` writes only into the object database.
//!
//! Putting the files back reads the same tree: `git cat-file blob <tree>:<path>` for a file that
//! was there, and a removal for one the turn created. The set is the difference between the tree
//! and the worktree now — never the transcript's tool targets, which miss a file written by
//! `patch` or by a shell call.
//!
//! A boundary restore is not a merge: an edit made by hand *after* that turn is reverted too, so
//! every path is named as it is put back.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::process;
use crate::status::GIT_TIMEOUT;

mod undo;
pub use undo::Undo;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;

/// How one path differs from the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The path as git writes it: relative to the root, `/`-separated.
    pub path: String,
    /// The mode the snapshot holds for it, or `None` when the snapshot has no such file.
    pub mode: Option<u32>,
}

/// The worktree as a tree object, built in a scratch index under the repository's git directory.
///
/// The person's index file is never opened and no ref moves, which is what keeps `git status`
/// byte for byte what it was before the capture.
pub fn capture(root: &Path) -> Result<String, String> {
    let index = scratch_index(root)?;
    let tree = stage(root, &index);
    // The scratch index and the lock git may have left beside it, whether staging worked or not.
    let _ = std::fs::remove_file(&index);
    let _ = std::fs::remove_file(index.with_extension("lock"));
    tree
}

/// The paths that differ between `tree` and the worktree now, in the order git lists them.
pub fn changes(root: &Path, tree: &str) -> Result<Vec<Change>, String> {
    let now = capture(root)?;
    if now == tree {
        return Ok(Vec::new());
    }
    let raw = git(
        root,
        None,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--raw",
            "--no-commit-id",
            tree,
            &now,
        ],
    )?;
    Ok(parse(&String::from_utf8_lossy(&raw)))
}

/// A scratch index path in `root`'s git directory, unique to this process and moment.
fn scratch_index(root: &Path) -> Result<PathBuf, String> {
    let dir = git(root, None, &["rev-parse", "--absolute-git-dir"])?;
    let dir = String::from_utf8_lossy(&dir).trim().to_string();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    Ok(Path::new(&dir).join(format!("perch-index-{}-{stamp}", std::process::id())))
}

/// Stages the whole worktree into the scratch index and writes it out as a tree.
fn stage(root: &Path, index: &Path) -> Result<String, String> {
    git(root, Some(index), &["add", "--all"])?;
    let tree = git(root, Some(index), &["write-tree"])?;
    Ok(String::from_utf8_lossy(&tree).trim().to_string())
}

/// One git subcommand's stdout; `Err` is why it could not be read.
fn git(root: &Path, index: Option<&Path>, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .arg("--no-optional-locks")
        .args(args);
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let captured =
        process::run(cmd, GIT_TIMEOUT).map_err(|error| format!("git {}: {error}", args[0]))?;
    if !captured.ok {
        return Err(format!(
            "git {} failed: {}",
            args[0],
            captured.stderr.trim()
        ));
    }
    Ok(captured.stdout)
}

/// The `Change`s a `diff-tree --raw -z` record names.
///
/// A record is `:<old-mode> <new-mode> <old-sha> <new-sha> <status>` and the path follows it as
/// its own NUL-terminated field.
fn parse(raw: &str) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut fields = raw.split('\0');
    while let Some(header) = fields.next() {
        if !header.starts_with(':') {
            continue;
        }
        let Some(path) = fields.next() else {
            break;
        };
        let mut modes = header.trim_start_matches(':').split(' ');
        let old = modes
            .next()
            .and_then(|mode| u32::from_str_radix(mode, 8).ok())
            .unwrap_or(0);
        changes.push(Change {
            path: path.to_string(),
            mode: (old != 0).then_some(old),
        });
    }
    changes
}

/// Puts `changes` back to what `tree` holds, answering one line per path.
fn put_back(root: &Path, tree: &str, changes: &[Change]) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for change in changes {
        let target = root.join(&change.path);
        match change.mode {
            // Absent from the snapshot, so the turn created it: putting the turn back removes it.
            None => {
                std::fs::remove_file(&target)
                    .map_err(|error| format!("{} could not be removed: {error}", change.path))?;
                lines.push(format!("removed {}", change.path));
            }
            Some(mode) => {
                let bytes = blob(root, tree, &change.path)?;
                write(&target, &bytes, mode)?;
                lines.push(format!("restored {}", change.path));
            }
        }
    }
    Ok(lines)
}

/// The bytes `tree` holds for `path`.
fn blob(root: &Path, tree: &str, path: &str) -> Result<Vec<u8>, String> {
    let spec = format!("{tree}:{path}");
    git(root, None, &["cat-file", "blob", &spec])
}

/// Writes `bytes` at `target`, with the mode the snapshot held.
fn write(target: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{} could not be created: {error}", parent.display()))?;
    }
    std::fs::write(target, bytes)
        .map_err(|error| format!("{} could not be written: {error}", target.display()))?;
    std::fs::set_permissions(target, std::fs::Permissions::from_mode(mode & 0o777)).map_err(
        |error| {
            format!(
                "{} could not be made {:o}: {error}",
                target.display(),
                mode & 0o777
            )
        },
    )
}
