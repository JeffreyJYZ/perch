//! Which directory the window opens on.
//!
//! A session belongs to a worktree that already exists, and the launch directory is the one thing
//! that is always true without reading anything — but only when it is a project. Launching from
//! Finder gives `/`, and treating that as a worktree makes the app walk the whole disk: slow, and
//! it trips every privacy prompt macOS has. So a directory that is a filesystem root, a user
//! directory, or a home directory is refused with words instead.

use std::path::{Path, PathBuf};

/// Why a directory cannot be a worktree.
pub fn refusal(path: &Path) -> Option<String> {
    refusal_with(path, std::env::var_os("HOME").map(PathBuf::from).as_deref())
}

/// The same rule with the home directory handed in, so a test needs no environment of its own.
pub fn refusal_with(path: &Path, home: Option<&Path>) -> Option<String> {
    if path.parent().is_none() {
        return Some(format!(
            "{} is the filesystem root, not a project; pass --worktree <path>",
            path.display()
        ));
    }
    if let Some(home) = home {
        if path == home {
            return Some(format!(
                "{} is your home directory, not a project; pass --worktree <path>",
                path.display()
            ));
        }
        if home.starts_with(path) {
            return Some(format!(
                "{} contains your home directory; pass --worktree <path>",
                path.display()
            ));
        }
    }
    None
}

/// The session list a window with no app draws: one session on the project, or none when the
/// launch directory is not one. A window with no session shows the empty regions rather than a
/// made-up worktree.
pub fn fallback_sessions(flag: Option<PathBuf>) -> Vec<perch_store::SessionMeta> {
    let Ok(worktree) = project_root(flag) else {
        return Vec::new();
    };
    let title = worktree.file_name().map_or_else(
        || "perch".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    vec![perch_store::SessionMeta {
        id: "current".to_string(),
        title,
        worktree,
        branch: String::new(),
        model: "no model".to_string(),
        created: 0,
    }]
}

/// The directory the window opens on: the flag when given, otherwise the launch directory.
///
/// Every answer is canonicalised first, so `/private/var/...` and `/var/...` cannot disagree
/// about which path was refused.
pub fn project_root(flag: Option<PathBuf>) -> Result<PathBuf, String> {
    let candidate = match flag {
        Some(path) => path,
        None => std::env::current_dir().map_err(|error| format!("no launch directory: {error}"))?,
    };
    let path = candidate
        .canonicalize()
        .map_err(|error| format!("{} is not readable: {error}", candidate.display()))?;
    if let Some(why) = refusal(&path) {
        return Err(why);
    }
    if !path.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    Ok(path)
}
