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

/// The directory the window opens on when nothing said which: the launch directory, refused when
/// it is not a project.
///
/// An explicit choice is never refused: `--worktree` and the folder picker say what a person
/// wants, and a session on a home directory or a root is then their decision to make. Only the
/// guess is guarded, because walking `/` is never what anyone meant.
pub fn project_root(flag: Option<PathBuf>) -> Result<PathBuf, String> {
    project_root_with(flag, std::env::current_dir().ok())
}

/// What a launch opens.
///
/// A directory that is not a project is a window with no folder and the picker, never a window
/// that refuses to open: the person can still choose one once the window is there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    /// The directory is a project, so the window starts a session on it.
    Project(PathBuf),
    /// There is nothing to open, and this is why, in words.
    Empty(String),
}

/// The launch decision with the launch directory and home handed in, so a test needs no
/// environment of its own.
///
/// The flag is the launch directory when it is given; without one the directory the process
/// started in is used, which is `/` for a window opened from Finder.
pub fn launch_with(flag: Option<PathBuf>, cwd: Option<PathBuf>, home: Option<&Path>) -> Launch {
    let Some(candidate) = flag.or(cwd) else {
        return Launch::Empty("started with no launch directory".to_string());
    };
    let Ok(path) = candidate.canonicalize() else {
        return Launch::Empty(format!(
            "{} is not readable; open a folder in the window",
            candidate.display()
        ));
    };
    if let Some(why) = refusal_with(&path, home) {
        return Launch::Empty(why);
    }
    if !path.is_dir() {
        return Launch::Empty(format!(
            "{} is not a directory; open a folder in the window",
            path.display()
        ));
    }
    Launch::Project(path)
}

/// The window's launch: the flag when given, otherwise where the process started.
pub fn launch(flag: Option<PathBuf>) -> Launch {
    launch_with(
        flag,
        std::env::current_dir().ok(),
        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
    )
}

/// The same rule with the launch directory handed in as well.
fn project_root_with(flag: Option<PathBuf>, cwd: Option<PathBuf>) -> Result<PathBuf, String> {
    match flag {
        Some(path) => directory(path),
        None => {
            let launch =
                std::env::current_dir().map_err(|error| format!("no launch directory: {error}"))?;
            let path = directory(launch)?;
            match refusal(&path) {
                Some(why) => Err(why),
                None => Ok(path),
            }
        }
    }
}

/// A canonical directory, or words naming why it cannot be one.
pub fn directory(candidate: PathBuf) -> Result<PathBuf, String> {
    let path = candidate
        .canonicalize()
        .map_err(|error| format!("{} is not readable: {error}", candidate.display()))?;
    if !path.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    Ok(path)
}
