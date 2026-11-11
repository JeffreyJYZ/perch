//! The project's own instructions, carried in the session's system message.
//!
//! Every `AGENTS.md` and `CLAUDE.md` from the session's worktree up to `/`, outermost first, is
//! read by the agent crate, which caps each file and the total and says where it cut. This module
//! only decides how that text joins the system message, and when it is big enough to be worth a
//! line in the column.

use std::path::Path;

use perch_agent::for_session;

use super::super::SYSTEM;

/// The size of project instructions that stops being a note and starts being the context.
const BUDGET: usize = 32 * 1024;

/// The system message for a session in `worktree`, and one line when it is very large.
///
/// The worktree is the session's own, never the data directory: a session opened on another
/// repository must be told that repository's rules. The text is sent whole — the agent crate has
/// already capped it, and dropping a file here would be the app deciding something the person
/// cannot see — so a message carrying too much says so rather than losing a file.
pub(super) fn system(worktree: &Path) -> (String, Option<String>) {
    let instructions = for_session(worktree);
    if instructions.is_empty() {
        return (SYSTEM.to_string(), None);
    }
    let note = (instructions.len() > BUDGET).then(|| {
        format!(
            "project instructions are {} bytes and every request carries them; trim a file in {} \
             if that is too much context",
            instructions.len(),
            worktree.display()
        )
    });
    (format!("{SYSTEM}\n\n{instructions}"), note)
}
