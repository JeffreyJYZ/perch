//! Project instructions, read from the worktree up to the filesystem root.

use std::path::Path;

use perch_agent::instructions::{MAX_FILE_BYTES, MAX_TOTAL_BYTES, for_worktree};

/// One directory with a file in it, and the directory below it.
fn tree(root: &Path) {
    std::fs::create_dir_all(root.join("outer/mid/app")).unwrap();
}

#[test]
fn a_file_two_directories_up_and_one_in_the_worktree_both_appear_deepest_last() {
    let tmp = tempfile::tempdir().unwrap();
    let worktree = tmp.path().join("outer/mid/app");
    tree(tmp.path());
    std::fs::write(tmp.path().join("outer/AGENTS.md"), "rule from two up").unwrap();
    std::fs::write(worktree.join("AGENTS.md"), "rule from the worktree").unwrap();

    let text = for_worktree(&worktree);

    let up = text
        .find("rule from two up")
        .expect("the upper file is read");
    let deep = text
        .find("rule from the worktree")
        .expect("the worktree file is read");
    assert!(
        up < deep,
        "outermost first, so the deepest file has the last word"
    );
    assert!(
        text.contains(&tmp.path().join("outer/AGENTS.md").display().to_string()),
        "each block names the file it came from:\n{text}"
    );
}

#[test]
fn claude_md_is_read_after_agents_md_in_the_same_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("AGENTS.md"), "agents rule").unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), "claude rule").unwrap();

    let text = for_worktree(tmp.path());

    assert!(text.find("agents rule").unwrap() < text.find("claude rule").unwrap());
}

#[test]
fn a_forty_kib_file_is_cut_and_the_cut_is_named() {
    let tmp = tempfile::tempdir().unwrap();
    let body = "a".repeat(40 * 1024);
    std::fs::write(tmp.path().join("AGENTS.md"), &body).unwrap();

    let text = for_worktree(tmp.path());

    assert!(
        text.contains("AGENTS.md"),
        "the cut names the file:\n{text}"
    );
    assert!(
        text.contains(&format!("only the first {MAX_FILE_BYTES}")),
        "the cut states how much was read:\n{text}"
    );
    assert!(text.contains("40960"), "and how long the file is");
    assert!(
        text.len() < body.len() + 1024,
        "the assembly cannot be the whole file"
    );
}

#[test]
fn the_total_cap_names_a_file_it_could_not_include() {
    let tmp = tempfile::tempdir().unwrap();
    let one = "b".repeat(MAX_FILE_BYTES);
    std::fs::write(tmp.path().join("AGENTS.md"), &one).unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), &one).unwrap();

    // Four 32 KiB files cannot fit 96 KiB, so the last one is cut or excluded rather than
    // silently dropped.
    std::fs::create_dir_all(tmp.path().join("deep")).unwrap();
    std::fs::write(tmp.path().join("deep/AGENTS.md"), &one).unwrap();
    std::fs::write(tmp.path().join("deep/CLAUDE.md"), &one).unwrap();

    let text = for_worktree(&tmp.path().join("deep"));

    assert!(
        text.contains("98304 byte total"),
        "the total is stated, not silently applied:\n{}",
        &text[text.len().saturating_sub(600)..]
    );
    assert!(text.len() <= MAX_TOTAL_BYTES + 4096);
}

#[test]
fn a_whitespace_only_file_contributes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("AGENTS.md"), "  \n\t\n").unwrap();

    assert_eq!(for_worktree(tmp.path()), "");
}

#[test]
fn an_unreadable_path_yields_empty_not_an_error() {
    let missing = Path::new("/nonexistent-perch-instructions-test/app");
    assert_eq!(for_worktree(missing), "");
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_worktree_is_never_put_in_the_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let worktree = tmp.path().join("app");
    std::fs::create_dir_all(&worktree).unwrap();
    let secret = tmp.path().join("secret.txt");
    std::fs::write(&secret, "a key that is not an instruction").unwrap();
    std::os::unix::fs::symlink(&secret, worktree.join("AGENTS.md")).unwrap();

    let text = for_worktree(&worktree);

    assert!(
        !text.contains("a key that is not an instruction"),
        "a repository must not be able to point its instructions at a secret:\n{text}"
    );
}

#[cfg(unix)]
#[test]
fn a_claude_md_linked_to_agents_md_inside_the_worktree_is_read() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("AGENTS.md"), "the shared rule").unwrap();
    std::os::unix::fs::symlink("AGENTS.md", tmp.path().join("CLAUDE.md")).unwrap();

    let text = for_worktree(tmp.path());

    assert_eq!(
        text.matches("the shared rule").count(),
        2,
        "the link is the same file and is read like it:\n{text}"
    );
}

#[cfg(unix)]
#[test]
fn a_file_that_cannot_be_read_is_a_line_of_text() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("AGENTS.md");
    std::fs::write(&path, "unreadable rule").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let text = for_worktree(tmp.path());

    assert!(
        text.contains("could not be read") && text.contains("AGENTS.md"),
        "a read failure is stated instead of escaping:\n{text}"
    );
    assert!(!text.contains("unreadable rule"));
}
