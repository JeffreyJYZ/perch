//! The left column: the session list over the file tree, and the selected file's preview.

use perch_store::SessionMeta;
use rune::{El, col};
use rune_kit::Kit;
use rune_kit::shell::SidebarRow;
use rune_kit::{Button, ButtonVariant};

use super::ROW;
use super::live::Live;
use super::state::Shell;
use crate::files::{files_view, note, section_label};
use crate::preview::preview_view;

/// The session list, and the tab strip it is the long form of.
///
/// A session whose worktree is gone still has a row: the work is on disk, and hiding it would
/// hide the only place its reason is said. Starting a new one needs no picker — it is the
/// directory this window already has open.
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
        .child(section_label(kit, "Sessions", "sessions-label"));
    if shell.sessions.is_empty() {
        list = list.child(note(kit, "No sessions yet"));
    }
    for (index, session) in shell.sessions.iter().enumerate() {
        list = list.child(
            SidebarRow::new(session.title.clone())
                .height(ROW)
                .subtitle(describe(session))
                .selected(index == shell.active)
                .on_select(move |s: &mut Shell<L>, _: &mut rune::Tx| s.select(index))
                .build(kit)
                .key(format!("session-{index}")),
        );
    }
    list.child(new_session::<L>(kit))
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

/// The file tree, with the selected file's preview under it so the Zed handoff is reachable.
pub(super) fn files<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let Some(panel) = shell.files.as_ref() else {
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
            .child(note(kit, "not loaded"));
    };
    let tree = files_view::<L>(panel);
    // Nothing to preview and nothing to say: the column is the tree, as before.
    if panel.selected_path().is_none() && panel.message().is_none() {
        return tree;
    }
    col()
        .key("files-column")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(tree)
        .child(preview_view::<L>(panel))
}

/// A session row's second line: its branch and its worktree, both as words — or the reason the
/// session cannot run, before a prompt is typed rather than after one is refused.
fn describe(session: &SessionMeta) -> String {
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
