//! What `/undo` holds: the tree a turn started from, and putting its files back.

use std::path::Path;

use super::{capture, changes, put_back};

/// The worktree as it was when a turn started, kept beside the session that ran it.
#[derive(Debug, Default)]
pub struct Undo {
    /// The tree object of the moment the turn started, when one could be captured.
    tree: Option<String>,
    /// Whether the last turn's files were put back, so a second `/undo` says so instead of
    /// pretending there is nothing to do.
    restored: bool,
}

impl Undo {
    /// Captures the worktree as it is, for a turn that is about to run.
    ///
    /// A worktree that is not a repository, or a git call that fails, keeps no snapshot: `/undo`
    /// then names that rather than restoring something that was never recorded.
    pub fn capture(&mut self, worktree: &Path) {
        self.tree = capture(worktree).ok();
        self.restored = false;
    }

    /// Puts the last turn's files back, answering one line per path for the transcript.
    ///
    /// `running` is a turn in flight: nothing is restored under the agent, because its own writes
    /// are still landing.
    pub fn restore(&mut self, worktree: &Path, running: bool) -> Result<Vec<String>, String> {
        if running {
            return Err("a turn is running, so nothing was restored: stop it first".to_string());
        }
        let Some(tree) = self.tree.clone() else {
            if !crate::diff::is_repo(worktree) {
                return Err(format!(
                    "{} is not a git worktree, so a turn's start cannot be saved",
                    worktree.display()
                ));
            }
            if self.restored {
                return Err(
                    "the last turn's files were already put back, so there is nothing left to undo"
                        .to_string(),
                );
            }
            return Err(
                "no turn has started in this session, so there is nothing to restore".to_string(),
            );
        };
        let changes = changes(worktree, &tree)?;
        if changes.is_empty() {
            return Err(
                "nothing has changed since that turn began, so there is nothing to restore"
                    .to_string(),
            );
        }
        let lines = put_back(worktree, &tree, &changes)?;
        self.tree = None;
        self.restored = true;
        Ok(lines)
    }
}
