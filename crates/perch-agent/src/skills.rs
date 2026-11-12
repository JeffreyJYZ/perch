//! Skills: the instruction sets a person keeps for a kind of task, found by name.
//!
//! A skill is a directory holding a `SKILL.md`: front matter carrying `name` and `description`,
//! then the body — the instructions a turn follows once the skill is loaded. They live in two
//! places and both are read: the machine's own `~/.config/perch/skills`, then a `.perch/skills`
//! beside the project, every directory from the root down to the worktree, nearest last, so a
//! project overrides a machine-wide skill of the same name.
//!
//! Nothing here can fail a turn. A file with no front matter is passed over and counted, and a
//! body larger than its share is cut with the cut named in the text that carries it.

mod parse;

use std::path::{Path, PathBuf};

/// The most one body may contribute, so a generated document cannot fill the context.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// The file every skill directory carries.
pub const FILE: &str = "SKILL.md";

/// The machine's own skills, relative to the home directory.
const HOME: &str = ".config/perch/skills";

/// A project's skills, relative to any directory above the worktree.
const PROJECT: &str = ".perch/skills";

/// One skill: what it is called, what it is for, and the instructions it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    pub path: PathBuf,
}

/// What a sweep found: the skills, and how many files were passed over for having no front matter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub skills: Vec<Skill>,
    pub skipped: usize,
}

/// Every skill this session can ask for: the machine's own, then the project's, nearest last.
pub fn discover(worktree: &Path) -> Vec<Skill> {
    discover_in(Some(worktree), home_dir().as_deref()).skills
}

/// [`discover`] with the worktree and the home stated, which is what a test and a sandbox use so
/// neither ever reads the machine it happens to run on.
pub fn discover_in(worktree: Option<&Path>, home: Option<&Path>) -> Found {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = home {
        roots.push(home.join(HOME));
    }
    if let Some(worktree) = worktree {
        let mut dirs: Vec<&Path> = worktree.ancestors().collect();
        dirs.reverse();
        roots.extend(dirs.into_iter().map(|dir| dir.join(PROJECT)));
    }

    let mut found = Found::default();
    for root in roots {
        for path in files(&root) {
            match parse::read(&path) {
                Some(skill) => remember(&mut found.skills, skill),
                None => found.skipped += 1,
            }
        }
    }
    found
}

/// The skill named `name`, or `None` when nothing carries it.
pub fn load(worktree: &Path, name: &str) -> Option<Skill> {
    load_in(Some(worktree), home_dir().as_deref(), name)
}

/// [`load`] with the worktree and the home stated.
pub fn load_in(worktree: Option<&Path>, home: Option<&Path>, name: &str) -> Option<Skill> {
    discover_in(worktree, home)
        .skills
        .into_iter()
        .find(|skill| skill.name == name)
}

/// The home directory the machine's skills are looked for in: `PERCH_HOME` when a test or a
/// sandbox sets one, otherwise the real home — the same seam the instruction loader uses.
pub fn home_dir() -> Option<PathBuf> {
    crate::instructions_home::home()
}

/// The `SKILL.md` of every directory under `root`, in name order so the list never depends on the
/// order the disk happens to hand back.
fn files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path().join(FILE))
        .filter(|path| path.is_file())
        .collect();
    paths.sort();
    paths
}

/// Adds one skill, or replaces an earlier one of the same name where it already sits: the sweep
/// runs outermost first, so the entry landing later is the nearer file and has the final word.
fn remember(skills: &mut Vec<Skill>, skill: Skill) {
    match skills.iter_mut().find(|known| known.name == skill.name) {
        Some(known) => *known = skill,
        None => skills.push(skill),
    }
}
