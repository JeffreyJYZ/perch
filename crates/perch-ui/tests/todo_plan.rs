//! The plan panel: drawn from the table `todowrite` writes, and absent when the list is empty.
//!
//! The window is the real layout with a stand-in app whose only data is the todo table, so what is
//! asserted is the row a frame draws after a write — no reload, no rebuild, no second frame source.

use std::path::{Path, PathBuf};
use std::time::Duration;

use perch_store::SessionMeta;
use perch_tools::{ToolCtx, ToolRegistry};
use perch_ui::Shell;
use perch_ui::shell::{Live, Panels, Waiting};
use rune::{Env, Runtime};
use serde_json::{Value, json};

/// The app behind the window: this worktree's todo list, and nothing else.
struct App {
    root: PathBuf,
}

impl Live for App {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&perch_ui::Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Ok(())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
    fn todos(&self) -> Vec<perch_tools::Todo> {
        perch_tools::todos(&self.root)
    }
}

impl Panels for App {}

fn meta(worktree: &Path) -> SessionMeta {
    SessionMeta {
        id: "s".to_string(),
        title: "s".to_string(),
        worktree: worktree.to_path_buf(),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    }
}

/// The window over `worktree`, laid out once.
fn window(worktree: &Path) -> Runtime<Shell<App>> {
    let mut runtime = Runtime::new(
        Shell::new(vec![meta(worktree)], 0).with_live(App {
            root: worktree.to_path_buf(),
        }),
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 2.0),
    );
    runtime.frame(0.0);
    runtime
}

/// The node keyed `key`, wherever it sits in the tree.
fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}

/// What the model writes, through the tool the model calls.
fn write_todos(root: &Path, todos: Value) {
    ToolRegistry::builtin()
        .call(
            "todowrite",
            &json!({"todos": todos}),
            &ToolCtx {
                root: root.to_path_buf(),
            },
        )
        .expect("the write is allowed");
}

#[test]
fn a_frame_after_a_write_draws_each_status_and_an_empty_list_draws_no_rows() {
    let worktree = tempfile::tempdir().unwrap();
    let mut runtime = window(worktree.path());

    // Nothing written yet: no heading, no row, and no empty region claiming a plan exists.
    let blank = runtime.accessibility_json();
    assert!(
        find(&blank, "plan").is_none(),
        "an empty list drew a plan region: {blank}"
    );

    // The turn writes mid-run; the next frame is the only thing that happens before the assertion.
    write_todos(
        worktree.path(),
        json!([
            {"content": "read the spec", "status": "completed"},
            {"content": "write the tools", "status": "in_progress"},
            {"content": "draw the panel", "status": "pending"},
        ]),
    );
    runtime.invalidate();
    runtime.frame(1.0 / 60.0);
    assert_eq!(
        perch_tools::todos(worktree.path()).len(),
        3,
        "the write did not reach the table"
    );
    let drawn = runtime.accessibility_json();
    assert!(
        find(&drawn, "plan").is_some(),
        "the plan region was not drawn after a write: {drawn}"
    );
    let words = drawn.to_string();
    for word in [
        "PLAN",
        "[x]",
        "[•]",
        "[ ]",
        "read the spec",
        "write the tools",
        "draw the panel",
    ] {
        assert!(words.contains(word), "the panel is missing {word}: {words}");
    }
    for index in 0..3 {
        assert!(
            find(&drawn, &format!("todo-{index}")).is_some(),
            "todo {index} has no row: {drawn}"
        );
    }

    // The plan is under the file tree, not beside it, and it is given real height. The tree is
    // still springing back from the new sibling on the frame that first draws it, so the resting
    // layout is what is read here.
    runtime.settle_all();
    runtime.frame(1.0 / 60.0);
    let settled = runtime.accessibility_json();
    let plan = find(&settled, "plan").expect("the plan region is drawn");
    let files = find(&settled, "files").expect("the files region is drawn");
    let top = |node: &Value| node["bounds"][1].as_f64().unwrap_or(0.0);
    let height = |node: &Value| node["bounds"][3].as_f64().unwrap_or(0.0);
    assert!(
        top(plan) >= top(files) + height(files),
        "the plan is not under the files region: plan {plan}, files {files}"
    );
    assert!(
        height(plan) > 0.0,
        "the plan was squeezed to no height: {plan}"
    );

    // Clearing the list clears the panel: a label with nothing under it is the empty state this
    // panel does not draw.
    write_todos(worktree.path(), json!([]));
    runtime.invalidate();
    runtime.frame(1.0 / 60.0);
    let cleared = runtime.accessibility_json();
    assert!(
        find(&cleared, "plan").is_none(),
        "a cleared list still drew a plan region: {cleared}"
    );
    assert!(
        find(&cleared, "todo-0").is_none(),
        "a cleared list still drew a row: {cleared}"
    );
}
