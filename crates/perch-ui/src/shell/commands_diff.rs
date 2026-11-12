//! `/diff`: the files the last finished turn changed, and each one's diff.
//!
//! The set is the transcript's own record — the targets its writing cards named between one
//! closing summary and the next — never `git status`, which answers a different question and
//! would list every file a person changed by hand. A card's target is written by the model, so
//! every one is confined the way the tools confine themselves before a diff is read: a target
//! outside the worktree never reaches the diff, and is refused in words rather than skipped.
//!
//! The answer is notes, not a reply: a note is the column's system voice, and the diff is not
//! something the model said.

use std::path::{Path, PathBuf};

use perch_files::{file_diff, is_repo};

use super::live::Live;
use super::state::Shell;

/// How many files one `/diff` lists before it says how many it left out.
const SHOWN: usize = 10;

/// How many lines of one file's diff are said. The column draws a note, and a whole diff would
/// bury the list of files that follows it.
const LINES: usize = 120;

impl<L: Live> Shell<L> {
    /// `/diff [path]`: every file the last finished turn changed, or one of them.
    pub(super) fn command_diff(&mut self, arg: &str) -> Result<Option<String>, String> {
        let root = self
            .active_session()
            .map(|session| session.worktree.clone())
            .ok_or_else(|| "diff: no session is open, so there is nothing to diff".to_string())?;
        // The session's worktree as it really is, so a path read back from git compares equal.
        let root = root.canonicalize().unwrap_or(root);
        if !is_repo(&root) {
            return Err(format!(
                "diff: {} is not a git worktree, so there is no diff to read",
                root.display()
            ));
        }
        let Some(targets) = self.transcript.last_turn_files() else {
            return Err(
                "diff: no finished turn is in the transcript yet, so there is nothing to diff"
                    .to_string(),
            );
        };
        let (changed, refused) = self.turn_changes(&root, &targets);
        for target in &refused {
            self.say(&format!(
                "diff: {target} is outside the worktree, so it was not read"
            ));
        }
        let asked = arg.trim();
        if !asked.is_empty() {
            return self.diff_one(&root, asked, &changed);
        }
        if changed.is_empty() {
            return Ok(Some("diff: the last turn changed no file".to_string()));
        }
        let names: Vec<String> = changed
            .iter()
            .map(|(path, _)| relative(&root, path))
            .collect();
        self.say(&format!(
            "diff: {} file(s) changed by the last turn: {}",
            names.len(),
            names.join(", ")
        ));
        for (path, text) in changed.iter().take(SHOWN) {
            self.say(&format!("--- {}\n{}", relative(&root, path), cut(text)));
        }
        if names.len() > SHOWN {
            self.say(&format!(
                "diff: {} more file(s) not shown; /diff <path> reads one",
                names.len() - SHOWN
            ));
        }
        Ok(None)
    }

    /// The targets that are files this worktree changed, and the ones that escaped it.
    fn turn_changes(
        &self,
        root: &Path,
        targets: &[String],
    ) -> (Vec<(PathBuf, String)>, Vec<String>) {
        let mut changed: Vec<(PathBuf, String)> = Vec::new();
        let mut refused: Vec<String> = Vec::new();
        for target in targets {
            let Ok(path) = perch_tools::confine(root, Path::new(target)) else {
                // A card's target is the model's words, read only where the tools may read.
                refused.push(target.clone());
                continue;
            };
            if changed.iter().any(|(held, _)| *held == path) {
                continue;
            }
            if let Some(text) = file_diff(root, &path) {
                changed.push((path, text));
            }
        }
        (changed, refused)
    }

    /// One file of that turn, or the refusal that says it was not one of them.
    fn diff_one(
        &mut self,
        root: &Path,
        asked: &str,
        changed: &[(PathBuf, String)],
    ) -> Result<Option<String>, String> {
        let Ok(path) = perch_tools::confine(root, Path::new(asked)) else {
            return Err(format!(
                "diff: {asked} is outside the worktree, so there is nothing to read"
            ));
        };
        let Some((_, text)) = changed.iter().find(|(held, _)| *held == path) else {
            return Err(format!(
                "diff: {asked} was not changed by the last turn, so there is no diff of it here"
            ));
        };
        self.say(&format!("--- {asked}\n{}", cut(text)));
        Ok(None)
    }
}

/// The path as a person reads it: relative to the worktree when it is inside it.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// The first [`LINES`] lines of a diff, and how many were left, so a note stays readable.
fn cut(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= LINES {
        return text.to_string();
    }
    format!(
        "{}\n[diff cut: {} of {} lines shown]",
        lines[..LINES].join("\n"),
        LINES,
        lines.len()
    )
}
