//! Where one worktree's prompt history file lives: a fingerprint of its path, under one directory.
//!
//! A separate file from [`history`](crate::history) because naming a file and ranking prompts are
//! two jobs, and a long worktree path cannot be a filename on its own.

use std::path::{Path, PathBuf};

/// The file the history for `worktree` is kept in, under `dir`.
pub fn path_in(dir: &Path, worktree: &Path) -> PathBuf {
    dir.join(format!("{:016x}.json", fingerprint(worktree)))
}

/// FNV-1a over the path's bytes: it names a file and decides nothing else, so stability across
/// runs is what matters and a cryptographic hash would not.
fn fingerprint(worktree: &Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in worktree.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
