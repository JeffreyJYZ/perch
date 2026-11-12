//! The plan: the model's todo list, read-only, under the file tree.
//!
//! The model owns the list — `todowrite` replaces it and `todoread` reads it — so nothing here is
//! pressable: a click would edit a plan the model would then be working from a stale copy of. The
//! marks are the whole of it, and a row whose status is `in_progress` takes the signal colour.
//!
//! An empty list draws no region at all rather than a heading with nothing under it.

use perch_tools::{Todo, TodoStatus};
use rune::{El, col, row, text};
use rune_kit::Kit;

use super::live::Live;
use super::state::Shell;
use crate::files::section_label;

/// The plan section, or `None` when there is no list to show.
pub(super) fn plan<L: Live>(shell: &Shell<L>, kit: &Kit) -> Option<El> {
    let todos = shell.todos();
    if todos.is_empty() {
        return None;
    }
    let mut body = col()
        .key("plan")
        .role("region")
        .label("Plan")
        .w_full()
        .shrink()
        .px(10.0)
        .pt(8.0)
        .pb(10.0)
        .gap(3.0)
        .child(section_label(kit, "Plan", "plan-label"));
    for (index, todo) in todos.iter().enumerate() {
        body = body.child(row_for(todo, index, kit));
    }
    Some(body)
}

/// One todo: its mark, then its words. The mark is a word as well as a colour, so the shape of the
/// row survives a screenshot, a colour-blind reader and the accessibility tree.
fn row_for(todo: &Todo, index: usize, kit: &Kit) -> El {
    let (mark, ink) = match todo.status {
        TodoStatus::Completed => ("[x]", kit.c.text_faint),
        TodoStatus::InProgress => ("[•]", kit.c.text_accent),
        TodoStatus::Pending => ("[ ]", kit.c.text_faint),
    };
    let words = match todo.status {
        TodoStatus::Completed => kit.c.text_faint,
        TodoStatus::Pending | TodoStatus::InProgress => kit.c.text_base,
    };
    row()
        .key(format!("todo-{index}"))
        .role("listitem")
        .label(format!("{mark} {}", todo.content))
        .w_full()
        .gap(6.0)
        .py(1.0)
        .child(
            text(mark)
                .key(format!("todo-mark-{index}"))
                .font(kit.ty.mono)
                .color(ink),
        )
        .child(
            text(todo.content.clone())
                .key(format!("todo-content-{index}"))
                .font(kit.ty.text_small)
                .color(words),
        )
}

impl<L: Live> Shell<L> {
    /// The active session's todo list, read from the app each frame: the list changes inside a
    /// turn, so a copy kept by the window would show the plan the model has already moved past.
    pub fn todos(&self) -> Vec<Todo> {
        self.live.as_ref().map(Live::todos).unwrap_or_default()
    }
}
