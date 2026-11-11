//! What a child turn is, as the tool sees it: one fallible answer, and how much it cost.

use perch_tools::ToolCtx;

use crate::cancel::Cancel;

/// One child turn: whatever runs a second agent loop and answers its final text.
pub trait Delegate: Send + Sync {
    /// Runs one child turn to completion and answers its final text, or why it produced none.
    ///
    /// `ctx` is the call's: the root a child reads in is the worktree the parent turn is bound to,
    /// and the tool cannot supply one of its own because it has no idea which session called it.
    ///
    /// `cancel` is the parent turn's; the child should take its own generation with
    /// [`Cancel::begin`] when it starts and stop when [`Cancel::stopped`] turns true, exactly as a
    /// turn does. Neither an empty answer nor a partial one is a result: both are an `Err` here.
    fn run(&self, task: &str, ctx: &ToolCtx, cancel: &Cancel) -> Result<String, String>;

    /// How many rounds the child's last turn took, when the delegate counts them.
    ///
    /// The round limit is the delegate's, so the count is too: this only decides whether the result
    /// line says `done in 2 rounds`. A delegate that does not count answers `None`.
    fn rounds(&self) -> Option<usize> {
        None
    }
}
