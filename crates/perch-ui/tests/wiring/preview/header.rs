//! The header strip and the line gutter: what the preview says about the file it shows.

use perch_files::Workspace;
use perch_ui::Shell;
use perch_ui::preview::RENDER_LINES;

use crate::harness::{click, find, meta, temp_repo_with, window};

use super::probe::bounds;

#[test]
fn the_header_names_the_file_and_its_language_and_offers_the_way_back() {
    let repo = temp_repo_with(&[("main.rs", "fn main() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));

    click(&mut runtime, "file-row-0");
    let tree = runtime.accessibility_json();
    let path = find(&tree, "preview-path").expect("the header names the file");
    assert!(
        path.to_string().contains("main.rs"),
        "the selected file's path is in the header: {path}"
    );
    let language = find(&tree, "preview-language").expect("the header names the language");
    assert!(
        language.to_string().contains("rust"),
        "the file's language is in the header: {language}"
    );
    let back = find(&tree, "preview-close").expect("the header offers the way back");
    assert!(
        back.to_string().contains("Back to transcript"),
        "the button says what it does: {back}"
    );
    assert!(
        tree.to_string().contains("Esc"),
        "the header says which key leaves the preview: {tree}"
    );
}

#[test]
fn a_file_longer_than_the_cap_says_so_in_the_header_and_stops_drawing() {
    let body: String = (1..=RENDER_LINES + 10)
        .map(|n| format!("line {n}\n"))
        .collect();
    let repo = temp_repo_with(&[("long.rs", &body)]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));

    click(&mut runtime, "file-row-0");
    let tree = runtime.accessibility_json();
    let cap = find(&tree, "preview-cap").expect("the header states the cap");
    assert!(
        cap.to_string()
            .contains(&format!("first {RENDER_LINES} of {}", RENDER_LINES + 10)),
        "the header says how many lines of how many: {cap}"
    );
    assert!(
        find(&tree, &format!("preview-line-{RENDER_LINES}")).is_some(),
        "the last line the cap allows is drawn"
    );
    assert!(
        find(&tree, &format!("preview-line-{}", RENDER_LINES + 1)).is_none(),
        "the line past the cap is not drawn"
    );
}

#[test]
fn line_numbers_share_one_fixed_gutter_left_of_the_code() {
    let repo = temp_repo_with(&[("a.rs", "fn a() {}\nfn b() {}\nfn c() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let mut runtime = window(Shell::with_workspace(vec![meta("work")], 0, workspace));

    click(&mut runtime, "file-row-0");
    let first = bounds(&runtime, "preview-number-1");
    let right_edge = first[0] + first[2];
    for line in 1..=3 {
        let number = bounds(&runtime, &format!("preview-number-{line}"));
        let code = bounds(&runtime, &format!("preview-text-{line}"));
        assert!(
            (number[2] - first[2]).abs() < 0.5,
            "line {line}'s gutter is {number:?}, the first is {first:?}"
        );
        assert!(
            ((number[0] + number[2]) - right_edge).abs() < 0.5,
            "line {line}'s number ends at {}, not {right_edge}: {number:?}",
            number[0] + number[2]
        );
        assert!(
            number[0] + number[2] < code[0],
            "line {line}'s number {number:?} sits left of its code {code:?}"
        );
    }
}
