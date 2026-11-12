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

#[cfg(test)]
mod tests {
    use super::system;
    use crate::app::SYSTEM;

    /// The tests read no machine-wide instructions: this machine's own `AGENTS.md` would join
    /// every message and decide whether these assertions hold. `PERCH_HOME` is the loader's seam.
    fn hermetic() {
        static HOME: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        let home = HOME.get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            // Safety: set once per test process, before any session is built.
            unsafe { std::env::set_var("PERCH_HOME", dir.path()) };
            dir
        });
        assert!(home.path().is_dir(), "the emptying home exists");
    }

    #[test]
    fn a_worktree_without_instructions_sends_the_system_message_alone() {
        hermetic();
        let dir = tempfile::tempdir().unwrap();
        let (message, note) = system(dir.path());
        assert_eq!(
            message, SYSTEM,
            "an app with nothing to add changed the message"
        );
        assert!(
            note.is_none(),
            "nothing was added, so there is nothing to say"
        );
    }

    #[test]
    fn a_worktree_with_instructions_carries_them_after_a_blank_line() {
        hermetic();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "Use tabs.").unwrap();
        let (message, note) = system(dir.path());
        assert!(
            message.starts_with(SYSTEM),
            "the system message was replaced: {message}"
        );
        assert!(
            message.contains(&format!("{SYSTEM}\n\n")),
            "no blank line between the system message and the file: {message}"
        );
        assert!(
            message.ends_with("Use tabs."),
            "the file's text is not last: {message}"
        );
        assert!(
            note.is_none(),
            "one small file is not worth a line: {note:?}"
        );
    }
}
