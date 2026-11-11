//! Project instructions: `AGENTS.md` and `CLAUDE.md`, read from the worktree up to the root.
//!
//! A project's conventions live in files beside its code, and the model only follows them when
//! they are part of the system prompt. Every directory from the session's worktree to the
//! filesystem root is visited outermost first, so the file nearest the work is read last and has
//! the final word when two of them disagree.
//!
//! Nothing here can fail a turn. A missing file contributes nothing, an unreadable one becomes a
//! line the model can read, and a file larger than its share is cut rather than refused, with the
//! cut stated in the text that names the file — a silently truncated instruction is worse than an
//! absent one, because the model cannot tell which half it is missing.
//!
//! A symlinked instruction file is read only when it resolves inside the worktree, so a repository
//! cannot point its own `AGENTS.md` at a file outside it and have that file put in the prompt.

use std::path::{Path, PathBuf};

/// The most one file may contribute, so a generated document cannot fill the context.
pub const MAX_FILE_BYTES: usize = 32 * 1024;

/// The most the whole assembly may contribute, across every file.
pub const MAX_TOTAL_BYTES: usize = 96 * 1024;

/// The names a project uses for its instructions, in the order they are read.
const NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// Every instruction file above `worktree`, assembled into one string for the system prompt.
///
/// Empty when there is none, so a caller can append the result without testing it first.
pub fn for_worktree(worktree: &Path) -> String {
    let mut dirs: Vec<PathBuf> = worktree.ancestors().map(Path::to_path_buf).collect();
    dirs.reverse();

    let mut out = String::new();
    for dir in dirs {
        for name in NAMES {
            let path = dir.join(name);
            // Only a file that exists is worth a word; a directory that never had one says
            // nothing, and saying it for every directory up to `/` would be most of the context.
            if !is_readable(&path, worktree) {
                continue;
            }
            match std::fs::read(&path) {
                Ok(bytes) => absorb(&mut out, &path, &bytes),
                Err(error) => push(
                    &mut out,
                    &format!(
                        "Instructions from {} could not be read: {error}",
                        path.display()
                    ),
                ),
            }
        }
    }
    out
}

/// Adds one file's content, cut to its share, to the assembly.
fn absorb(out: &mut String, path: &Path, bytes: &[u8]) {
    let used = out.len();
    let room = MAX_TOTAL_BYTES.saturating_sub(used);
    if room == 0 {
        push(
            out,
            &format!(
                "Instructions from {} were not read: the {} byte total was already reached",
                path.display(),
                MAX_TOTAL_BYTES
            ),
        );
        return;
    }

    let limit = MAX_FILE_BYTES.min(room);
    let end = floor_boundary(bytes, bytes.len().min(limit));
    let text = String::from_utf8_lossy(&bytes[..end]);
    if text.trim().is_empty() {
        return;
    }

    push(
        out,
        &format!("Instructions from {}:\n{}", path.display(), text),
    );

    if end < bytes.len() {
        let why = if limit < MAX_FILE_BYTES {
            format!("the {MAX_TOTAL_BYTES} byte total was reached")
        } else {
            format!("a file may contribute {MAX_FILE_BYTES} bytes")
        };
        push(
            out,
            &format!(
                "({path} is {len} bytes long; only the first {end} were read because {why}. \
                 The rest was cut.)",
                path = path.display(),
                len = bytes.len(),
            ),
        );
    }
}

/// Whether `path` is something this module would read: a file, or a link that stays in the work.
fn is_readable(path: &Path, worktree: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.file_type().is_symlink() {
        return meta.is_file();
    }
    // A link is followed only inside the worktree: a repository can point its `AGENTS.md` at a
    // file of its own, and must not be able to point it at a key or a history file.
    match (path.canonicalize(), worktree.canonicalize()) {
        (Ok(real), Ok(root)) => real.is_file() && real.starts_with(root),
        _ => false,
    }
}

/// The largest index at or below `end` that is a UTF-8 character boundary.
fn floor_boundary(bytes: &[u8], mut end: usize) -> usize {
    while end > 0 && end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    end
}

/// Appends one block, with a blank line between it and whatever came before.
fn push(out: &mut String, block: &str) {
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(block);
}
