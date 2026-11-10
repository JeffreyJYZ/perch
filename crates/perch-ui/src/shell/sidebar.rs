//! The left column: the session list over the file tree.

use perch_store::SessionMeta;
use rune::{El, col};
use rune_kit::Kit;
use rune_kit::shell::SidebarRow;

use super::ROW;
use super::live::Live;
use super::state::Shell;
use crate::files::{files_view, note, section_label};

/// The session list, and the tab strip it is the long form of.
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
    list
}

/// The file tree's slot: the worktree's rows when there is a workspace, and words when there is
/// not, so the region is never blank.
pub(super) fn files<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    match shell.files.as_ref() {
        Some(panel) => files_view(panel),
        None => col()
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
            .child(note(kit, "not loaded")),
    }
}

/// A session row's second line: its branch and its worktree, both as words.
fn describe(session: &SessionMeta) -> String {
    let branch = if session.branch.is_empty() {
        "no branch".to_string()
    } else {
        session.branch.clone()
    };
    let worktree = session.worktree.file_name().map_or_else(
        || session.worktree.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!("{branch} · {worktree}")
}
