//! The sidebar's session filter: one field narrows the list to the sessions whose title, worktree
//! or branch holds what is typed.
//!
//! Filtering is the list's view, never the app's: every session stays open, nothing is stopped,
//! reloaded or re-read, and which session is active does not move while the field narrows. Escape
//! clears the field and every row returns, the selection still on the session it was on — even
//! when the filter hid that row while it was being typed.

use perch_store::SessionMeta;
use rune::{Key, KeyEvent, TextEdit};

use super::live::Live;
use super::state::Shell;

/// The filter field, and nothing else: the rows it keeps are read from the sessions on demand, so
/// the field cannot go stale against a title the app rewrote.
#[derive(Default)]
pub(super) struct SessionFilter {
    pub(super) query: TextEdit,
}

/// Whether `session` holds `needle` in its title, its branch or its worktree path.
///
/// The needle is matched as a case-insensitive substring of each, so `dev` reaches a branch named
/// `dev` and a worktree under `~/dev`; an empty needle keeps every session.
#[must_use]
pub(super) fn matches(session: &SessionMeta, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    session.title.to_lowercase().contains(&needle)
        || session.branch.to_lowercase().contains(&needle)
        || session
            .worktree
            .to_string_lossy()
            .to_lowercase()
            .contains(&needle)
}

impl<L: Live> Shell<L> {
    /// What the filter field holds, as typed.
    pub fn session_filter_text(&self) -> &str {
        &self.session_filter.query.text
    }

    /// The sessions the filter keeps, each with the index its row selects.
    ///
    /// The index is the session's place in the whole list, not in the kept rows, so a row pressed
    /// while a filter is on selects the same session it named with the filter off.
    pub fn session_rows(&self) -> Vec<(usize, SessionMeta)> {
        let needle = self.session_filter_text().to_owned();
        self.headers()
            .into_iter()
            .enumerate()
            .filter(|(_, session)| matches(session, &needle))
            .collect()
    }

    /// How many rows of how many the list is showing, in words: `3 of 12 sessions`.
    pub fn session_count_words(&self) -> String {
        let total = self.sessions.len();
        if self.session_filter_text().trim().is_empty() {
            return match total {
                1 => "1 session".to_string(),
                count => format!("{count} sessions"),
            };
        }
        format!("{} of {total} sessions", self.session_rows().len())
    }

    /// Empties the filter, so every row is drawn again and the active session is still selected.
    ///
    /// The field keeps the keyboard if it had it: Escape clears the words rather than leaving the
    /// field, which is what a person typing a filter expects.
    pub fn clear_session_filter(&mut self) {
        let focused = self.session_filter.query.focused;
        self.session_filter.query = TextEdit::default();
        self.session_filter.query.focused = focused;
    }

    /// The filter field's own keys: Escape clears the field and leaves the rest to the app.
    pub fn session_filter_key(&mut self, ev: &KeyEvent) -> bool {
        if ev.mods.cmd || ev.mods.alt {
            return false;
        }
        if !matches!(&ev.key, Key::Escape) || self.session_filter_text().trim().is_empty() {
            return false;
        }
        self.clear_session_filter();
        true
    }
}

/// A bare Escape anywhere else in the window, with a filter still on, clears it.
///
/// Attached to the window's root, after every region that owns Escape has had it: an overlay
/// closes first, and only a press that would otherwise do nothing empties the field.
pub(super) fn escape_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.alt || !matches!(&ev.key, Key::Escape) {
        return false;
    }
    shell.session_filter_key(ev)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use perch_store::SessionMeta;

    use super::matches;

    fn session(title: &str, worktree: &str, branch: &str) -> SessionMeta {
        SessionMeta {
            id: title.to_string(),
            title: title.to_string(),
            worktree: PathBuf::from(worktree),
            branch: branch.to_string(),
            model: String::new(),
            created: 0,
        }
    }

    #[test]
    fn a_session_is_found_by_title_worktree_or_branch_whatever_the_case() {
        let alpha = session("Fix the Parser", "/nowhere/alpha-tools", "feature/login");
        assert!(matches(&alpha, "parser"), "the title, case-insensitively");
        assert!(matches(&alpha, "ALPHA"), "the worktree path");
        assert!(matches(&alpha, "feature"), "the branch");
        assert!(
            !matches(&alpha, "beta"),
            "a session matching none is kept out"
        );
        assert!(matches(&alpha, "  "), "an empty filter keeps every session");
    }
}
