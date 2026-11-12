//! Where the regions are: the preview in the centre, the tree alone in the sidebar, and no two
//! sidebar regions sharing a pixel.

use perch_files::Workspace;
use perch_ui::Shell;

use crate::harness::{click, find, meta, temp_repo_with, window};

use super::probe::{bounds, child_keys, shares_pixels};

#[test]
fn the_preview_never_shares_pixels_with_the_file_tree() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));

    click(&mut runtime, "file-row-0");
    let preview = bounds(&runtime, "preview");
    let tree = bounds(&runtime, "files");
    println!("preview {preview:?}, tree {tree:?}");
    assert!(
        !shares_pixels(preview, tree),
        "the preview {preview:?} and the tree {tree:?} must not overlap"
    );
    assert!(
        preview[2] > 0.0 && preview[3] > 0.0,
        "the preview is actually drawn: {preview:?}"
    );
    assert!(
        tree[2] > 0.0 && tree[3] > 0.0,
        "the tree is still drawn: {tree:?}"
    );
}

#[test]
fn no_two_regions_of_the_sidebar_share_pixels() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));
    click(&mut runtime, "file-row-0");

    let regions = child_keys(&runtime, "left-column");
    assert!(
        regions.len() >= 2,
        "the sidebar stacks its regions: {regions:?}"
    );
    for (index, first) in regions.iter().enumerate() {
        let a = bounds(&runtime, first);
        println!("{first} {a:?}");
        assert!(a[2] > 0.0 && a[3] > 0.0, "{first} is actually drawn: {a:?}");
        for second in &regions[index + 1..] {
            let b = bounds(&runtime, second);
            assert!(
                !shares_pixels(a, b),
                "{first} {a:?} and {second} {b:?} must not overlap"
            );
        }
    }
}

#[test]
fn the_sidebar_draws_the_tree_and_nothing_of_the_file() {
    let repo = temp_repo_with(&[("a.rs", "fn a_secret_body() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));

    click(&mut runtime, "file-row-0");
    let tree = runtime.accessibility_json();
    let sidebar = find(&tree, "left-column").expect("the sidebar exists");
    let shown = sidebar.to_string();
    assert!(
        !shown.contains("a_secret_body"),
        "no line of the file is drawn inside the sidebar: {shown}"
    );
    for key in ["preview-header", "preview-line-1", "open-in-zed"] {
        assert!(
            find(sidebar, key).is_none(),
            "{key} belongs to the centre, not the sidebar"
        );
    }
    assert!(
        shown.contains("a.rs") && shown.contains("selected"),
        "the tree keeps its words and its selection: {shown}"
    );
}
