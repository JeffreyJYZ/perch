//! A window opens on a project, never on a filesystem root or a home directory.

use std::path::Path;

use perch_app::launch::{project_root, refusal, refusal_with};

#[test]
fn a_filesystem_root_is_refused_with_the_flag_named() {
    let words = refusal_with(Path::new("/"), None).expect("root refused");
    assert!(words.contains("filesystem root"), "{words}");
    assert!(words.contains("--worktree"), "{words}");
}

#[test]
fn a_home_directory_and_anything_containing_it_are_refused() {
    let home = Path::new("/Users/someone");
    assert!(refusal_with(home, Some(home)).is_some());
    assert!(refusal_with(Path::new("/Users"), Some(home)).is_some());
    assert!(refusal_with(Path::new("/Users/someone/dev/app"), Some(home)).is_none());
}

#[test]
fn a_real_directory_is_accepted_and_a_missing_one_is_not() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project_root(Some(dir.path().to_path_buf())).expect("accepted");
    assert_eq!(root, dir.path().canonicalize().unwrap());
    assert!(project_root(Some(dir.path().join("gone"))).is_err());
}

#[test]
fn the_launch_directory_is_this_test_binarys_temporaries_at_most() {
    // Whatever the test's own cwd is, it must either be accepted or refused with words — never
    // silently turned into `/`.
    match project_root(None) {
        Ok(path) => assert_ne!(path, Path::new("/")),
        Err(words) => assert!(words.contains("--worktree"), "{words}"),
    }
    assert!(refusal(Path::new("/")).is_some());
}
