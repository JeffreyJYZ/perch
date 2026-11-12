//! The left column: the session list over the file tree.
//!
//! The column holds words and the tree, nothing else. A file's contents belong to the centre
//! column, which shows them at full width while a file is selected; drawing them here as well
//! put them in a strip built for folder names.

use perch_store::SessionMeta;
use rune::{El, col};
use rune_kit::Kit;
use rune_kit::{Button, ButtonVariant};

use super::live::Live;
use super::search_session_view;
use super::state::Shell;
use crate::files::{files_view, note, section_label};

/// The session list, and the tab strip it is the long form of.
///
/// A session whose worktree is gone still has a row: the work is on disk, and hiding it would
/// hide the only place its reason is said. Starting a new one needs no picker — it is the
/// directory this window already has open.
///
/// The filter field above the rows is the list's own view: it narrows what is drawn and leaves
/// every session open, its selection where it was.
///
/// A window that opened with no folder is not an empty window: the column says there is none,
/// repeats why when the launch directory was refused, and offers the picker.
pub(super) fn sessions<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let mut list = col()
        .key("sessions")
        .role("region")
        .label("Sessions")
        .w_full()
        .shrink()
        .px(10.0)
        .pt(12.0)
        .pb(8.0)
        .gap(2.0)
        .focusable(true)
        .on_key(super::keymap::session_list_key::<L>)
        .child(section_label(kit, "Sessions", "sessions-label"));
    if shell.session_count() == 0 {
        list = list.child(note(kit, "No folder open"));
        if let Some(words) = shell.notice() {
            list = list.child(note(kit, words));
        }
        return list.child(open_folder::<L>(kit));
    }
    list = list
        .child(search_session_view::filter_field(shell, kit))
        .child(search_session_view::count(shell, kit));
    match search_session_view::empty(shell) {
        Some(words) => list = list.child(note(kit, &words)),
        None => {
            for (index, session) in shell.session_rows() {
                list = list.child(search_session_view::row(shell, index, &session, kit));
            }
        }
    }
    // A notice is still news when a session is open: a `--worktree` that opened nothing says so
    // here rather than nowhere.
    if let Some(words) = shell.notice() {
        list = list.child(note(kit, words));
    }
    list.child(new_session::<L>(kit))
}

/// The button that asks for a folder, for a window that opened without one.
fn open_folder<L: Live>(kit: &Kit) -> El {
    col().key("open-folder-slot").w_full().pt(4.0).child(
        Button::new("Open folder")
            .help("Open a folder as this window's project")
            .on_press_tx(|shell: &mut Shell<L>, tx: &mut rune::Tx| shell.open_folder(tx))
            .build(kit)
            .key("open-folder"),
    )
}

/// The button that starts a session on the worktree already open.
fn new_session<L: Live>(kit: &Kit) -> El {
    col().key("new-session-slot").w_full().pt(4.0).child(
        Button::new("New session")
            .variant(ButtonVariant::GhostMuted)
            .help("Start a session on the worktree this window has open")
            .on_press(|shell: &mut Shell<L>| {
                shell.new_session();
            })
            .build(kit)
            .key("new-session"),
    )
}

/// The file tree, and nothing under it: the selected file is previewed in the centre column.
pub(super) fn files<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let Some(panel) = shell.files.as_ref() else {
        // A window with no folder has no tree to show, and "not loaded" would read as a failure.
        let words = if shell.sessions.is_empty() {
            "No folder open"
        } else {
            "not loaded"
        };
        return col()
            .key("files")
            .role("region")
            .label("Files")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .px(10.0)
            .pt(10.0)
            .pb(10.0)
            .gap(4.0)
            .child(section_label(kit, "Files", "files-label"))
            .child(note(kit, words));
    };
    files_view::<L>(panel, kit)
}

impl<L: Live> Shell<L> {
    /// The row's delete control: the first press arms it, the second removes the session.
    pub fn remove_session(&mut self, index: usize) -> bool {
        self.confirm_delete(index)
    }
}

/// A session row's second line: its branch and its worktree, both as words — or the reason the
/// session cannot run, before a prompt is typed rather than after one is refused.
pub(super) fn describe(session: &SessionMeta) -> String {
    let worktree = session.worktree.file_name().map_or_else(
        || session.worktree.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    if !session.worktree.exists() {
        return format!("missing · {worktree}");
    }
    let branch = if session.branch.is_empty() {
        "no branch".to_string()
    } else {
        session.branch.clone()
    };
    format!("{branch} · {worktree}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use perch_store::SessionMeta;

    use super::describe;

    fn meta(worktree: PathBuf) -> SessionMeta {
        SessionMeta {
            id: "s".to_string(),
            title: "s".to_string(),
            worktree,
            branch: "dev".to_string(),
            model: String::new(),
            created: 0,
        }
    }

    #[test]
    fn a_session_whose_worktree_is_gone_says_missing_in_words() {
        let gone = meta(PathBuf::from("/nowhere/perch-gone-worktree"));
        assert_eq!(describe(&gone), "missing · perch-gone-worktree");

        let here = tempfile::tempdir().unwrap();
        let live = meta(here.path().to_path_buf());
        let words = describe(&live);
        assert!(
            words.starts_with("dev · ") && !words.contains("missing"),
            "a live worktree names its branch instead: {words}"
        );
    }
}
