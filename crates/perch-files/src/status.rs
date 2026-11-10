//! Git state per file, read from `git status --porcelain`.
//!
//! `gix`'s status API needs feature flags plus a rewrite configuration just to report
//! deletions, and its item model still has to be folded into these five states. The
//! spec's own acceptance bar is "git state matches `git status`", so the porcelain
//! output *is* the contract; this module parses it with a 30 s timeout, both streams
//! captured, and no index write (`--no-optional-locks`, because the app never mutates
//! a worktree).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::process;

pub(crate) const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// The five states the tree can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Untracked,
    Conflicted,
}

/// Status for every path git reports, keyed relative to the workspace root.
///
/// A directory that is not a repository (or a machine with no `git`) yields no statuses
/// rather than an error, so a plain directory still browses.
pub(crate) fn status_map(root: &Path) -> HashMap<PathBuf, FileStatus> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root).arg("--no-optional-locks").args([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    ]);
    match process::run(cmd, GIT_TIMEOUT) {
        Ok(captured) if captured.ok => parse_porcelain(&captured.stdout),
        _ => HashMap::new(),
    }
}

fn classify(x: char, y: char) -> Option<FileStatus> {
    let both = |c: char| x == c || y == c;
    if x == '?' || y == '?' {
        return Some(FileStatus::Untracked);
    }
    if both('U') || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
        return Some(FileStatus::Conflicted);
    }
    if both('R') || both('C') {
        return Some(FileStatus::Added);
    }
    if both('A') {
        return Some(FileStatus::Added);
    }
    if both('D') {
        return Some(FileStatus::Deleted);
    }
    if both('M') {
        return Some(FileStatus::Modified);
    }
    None
}

/// Parses `--porcelain=v1 -z`: `XY <path>\0` per entry, and a rename or copy is followed
/// by its original path as a second field.
fn parse_porcelain(bytes: &[u8]) -> HashMap<PathBuf, FileStatus> {
    let mut map = HashMap::new();
    let mut fields = bytes.split(|b| *b == 0);
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let x = field[0] as char;
        let y = field[1] as char;
        let Some(status) = classify(x, y) else {
            continue;
        };
        let path = PathBuf::from(OsStr::from_bytes(&field[3..]).to_owned());
        if matches!(x, 'R' | 'C') {
            fields.next();
        }
        map.insert(path, status);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_codes_fold_into_the_five_states() {
        let bytes = b"?? new.rs\0 M changed.rs\0UU both.rs\0 R renamed.rs\0old.rs\0";
        let map = parse_porcelain(bytes);
        assert_eq!(map.get(Path::new("new.rs")), Some(&FileStatus::Untracked));
        assert_eq!(
            map.get(Path::new("changed.rs")),
            Some(&FileStatus::Modified)
        );
        assert_eq!(map.get(Path::new("both.rs")), Some(&FileStatus::Conflicted));
        assert_eq!(map.get(Path::new("renamed.rs")), Some(&FileStatus::Added));
        assert_eq!(
            map.get(Path::new("old.rs")),
            None,
            "the rename source is not a live path"
        );
    }
}
