//! Which directory the window opens on.
//!
//! A session belongs to a worktree that already exists, and the launch directory is the one thing
//! that is always true without reading anything — but only when it is a project. Launching from
//! Finder gives `/`, and treating that as a worktree makes the app walk the whole disk: slow, and
//! it trips every privacy prompt macOS has ever shown. So the guess is guarded: a launch directory
//! that is a filesystem root, a user directory, or a home directory opens no session, and the
//! window asks for a folder instead.
//!
//! An explicit choice is never refused. `--worktree` and the folder picker say what a person
//! wants, and a session on a home directory or a root is then their decision: it is opened, and
//! macOS asks for whatever permission walking it needs.

use std::path::{Path, PathBuf};

/// Why an implicit launch directory cannot be a worktree.
pub fn refusal(path: &Path) -> Option<String> {
    refusal_with(path, std::env::var_os("HOME").map(PathBuf::from).as_deref())
}

/// The same rule with the home directory handed in, so a test needs no environment of its own.
pub fn refusal_with(path: &Path, home: Option<&Path>) -> Option<String> {
    if path.parent().is_none() {
        return Some(format!(
            "{} is the filesystem root, not a project; pass --worktree <path> or open a folder",
            path.display()
        ));
    }
    if let Some(home) = home {
        if path == home {
            return Some(format!(
                "{} is your home directory, not a project; pass --worktree <path> or open a folder",
                path.display()
            ));
        }
        if home.starts_with(path) {
            return Some(format!(
                "{} contains your home directory; pass --worktree <path> or open a folder",
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
        model: String::new(),
        created: 0,
    }]
}

/// A canonical directory: every answer is canonicalised, so `/private/var/...` and `/var/...`
/// cannot disagree about which path was named, and a folder a person picked is accepted wherever
/// it is.
pub fn directory(candidate: PathBuf) -> Result<PathBuf, String> {
    let path = candidate
        .canonicalize()
        .map_err(|error| format!("{} is not readable: {error}", candidate.display()))?;
    if !path.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    Ok(path)
}

/// The directory the window opens on: the flag when it names one, otherwise the launch directory.
///
/// The flag is a person's choice and is taken as given; only the launch directory is refused when
/// it is not a project.
pub fn project_root(flag: Option<PathBuf>) -> Result<PathBuf, String> {
    project_root_with(flag, std::env::current_dir().ok())
}

/// The same with the launch directory handed in, so a test needs no environment of its own.
pub fn project_root_with(flag: Option<PathBuf>, cwd: Option<PathBuf>) -> Result<PathBuf, String> {
    match flag {
        Some(path) => directory(path),
        None => {
            let launch = cwd.ok_or_else(|| "no launch directory".to_string())?;
            // The guess is refused before it is read: `/` and a home directory are the wrong
            // answer whether or not they happen to be readable here.
            match refusal(&launch) {
                Some(why) => Err(why),
                None => directory(launch),
            }
        }
    }
}

/// What a launch opens.
///
/// A launch directory that is not a project is a window with no folder and the picker, never a
/// window that refuses to open: the person can still choose one once the window is there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    /// The directory is a project, so the window starts a session on it.
    Project(PathBuf),
    /// There is nothing to open, and this is why, in words.
    Empty(String),
}

/// The window's launch: the flag when given, otherwise where the process started.
pub fn launch(flag: Option<PathBuf>) -> Launch {
    launch_with(
        flag,
        std::env::current_dir().ok(),
        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
    )
}

/// The launch decision with the launch directory and home handed in, so a test needs no
/// environment of its own.
pub fn launch_with(flag: Option<PathBuf>, cwd: Option<PathBuf>, home: Option<&Path>) -> Launch {
    match flag {
        Some(path) => match directory(path) {
            Ok(path) => Launch::Project(path),
            Err(words) => Launch::Empty(format!("{words}; open a folder in the window")),
        },
        None => {
            let Some(cwd) = cwd else {
                return Launch::Empty("started with no launch directory".to_string());
            };
            // Refused before it is read: the guess is wrong on its own, not because of what the
            // disk says about it today.
            if let Some(why) = refusal_with(&cwd, home) {
                return Launch::Empty(why);
            }
            match directory(cwd) {
                Err(words) => Launch::Empty(format!("{words}; open a folder in the window")),
                Ok(path) => Launch::Project(path),
            }
        }
    }
}
