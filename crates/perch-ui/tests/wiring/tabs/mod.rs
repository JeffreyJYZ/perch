//! The strip is the window's one navigation: a session, every file opened under it, and the
//! settings are tabs, and the one whose pane is drawn is the one marked active.
//!
//! The window is laid out through Rune, so a press here is a real press on the drawn tree: a file
//! row's press, a tab's press, and Escape, which is the way back to the transcript.

mod strip;

use perch_files::Workspace;
use perch_ui::Shell;
use perch_ui::shell::Pane;
use rune::Runtime;
use serde_json::Value;
use tempfile::TempDir;

use crate::harness::{click, meta, temp_repo_with, window};

/// The window over a repository holding `files`, with one session.
fn over(files: &[(&str, &str)]) -> (TempDir, Runtime<Shell>) {
    let repo = temp_repo_with(files);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    (
        repo,
        window(Shell::with_workspace(vec![meta("work")], 0, workspace)),
    )
}

/// Whether `node` holds a node keyed `key`, anywhere under it.
fn contains(node: &Value, key: &str) -> bool {
    node["key"] == key
        || node["children"]
            .as_array()
            .is_some_and(|children| children.iter().any(|child| contains(child, key)))
}

/// The tab drawn with `name`, which is the word a person reads on it.
fn tab_named<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    if node["role"] == "tab" && node["name"] == name {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| tab_named(child, name))
}

/// Every tab drawn with `state`, so "the active state must be one" can be counted.
fn tabs_in(runtime: &Runtime<Shell>, state: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![runtime.accessibility_json()];
    while let Some(node) = stack.pop() {
        let selected = node["states"]
            .as_array()
            .is_some_and(|states| states.iter().any(|word| word.as_str() == Some(state)));
        if node["role"] == "tab"
            && selected
            && let Some(key) = node["key"].as_str()
        {
            found.push(key.to_string());
        }
        if let Some(children) = node["children"].as_array() {
            stack.extend(children.iter().cloned());
        }
    }
    found.sort();
    found
}

/// The index of the tree row named `name`: the key its press is drawn under.
fn row_of(runtime: &Runtime<Shell>, name: &str) -> usize {
    runtime
        .state
        .files()
        .expect("the window has a tree")
        .rows()
        .iter()
        .position(|row| row.name == name)
        .unwrap_or_else(|| panic!("no row {name}"))
}

/// A press on the row named `name`, which is how a person opens a file.
fn open(runtime: &mut Runtime<Shell>, name: &str) {
    let index = row_of(runtime, name);
    click(runtime, &format!("file-row-{index}"));
}

/// The file's tab, which must be drawn because a file is a pane.
fn file_tab(runtime: &Runtime<Shell>, name: &str) -> Value {
    let tree = runtime.accessibility_json();
    tab_named(&tree, name)
        .unwrap_or_else(|| panic!("no tab named {name} in {tree}"))
        .clone()
}

#[test]
fn a_file_row_opens_a_closable_tab_named_after_the_file() {
    let (_repo, mut runtime) = over(&[("a.rs", "fn a() {}\n")]);
    open(&mut runtime, "a.rs");

    assert!(
        matches!(runtime.state.pane(), Pane::File(path) if path.ends_with("a.rs")),
        "the press made the file the pane: {:?}",
        runtime.state.pane()
    );
    let tab = file_tab(&runtime, "a.rs");
    assert_eq!(tab["role"], "tab");
    assert_eq!(tab["name"], "a.rs", "the tab is named after the file");
    assert!(contains(&tab, "close"), "a file's tab is closable: {tab}");
    assert_eq!(
        tabs_in(&runtime, "selected"),
        vec![tab["key"].as_str().unwrap().to_string()],
        "the tab the press opened is the active one"
    );
}

#[test]
fn a_second_file_opens_a_second_tab_and_a_third_press_focuses_the_first() {
    let (_repo, mut runtime) = over(&[("a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n")]);
    open(&mut runtime, "a.rs");
    open(&mut runtime, "b.rs");
    assert_eq!(runtime.state.file_tabs().len(), 2, "two files, two tabs");

    let tree = runtime.accessibility_json();
    let (a, b) = (
        tab_named(&tree, "a.rs").expect("a tab for a.rs"),
        tab_named(&tree, "b.rs").expect("a tab for b.rs"),
    );
    assert!(
        a["position"].as_u64() < b["position"].as_u64(),
        "the tabs follow the order they were opened: {} then {}",
        a["position"],
        b["position"]
    );
    assert_eq!(tabs_in(&runtime, "selected").len(), 1);

    // The first file again: its tab is focused, not a second one added.
    open(&mut runtime, "a.rs");
    assert_eq!(runtime.state.file_tabs().len(), 2, "no duplicate tab");
    assert!(
        matches!(runtime.state.pane(), Pane::File(path) if path.ends_with("a.rs")),
        "the focused tab is the pane: {:?}",
        runtime.state.pane()
    );
}
