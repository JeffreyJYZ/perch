//! A window opens on a project, never on a guessed filesystem root or home directory — and never
//! refuses a folder a person named.

use std::path::{Path, PathBuf};

use perch_app::launch::{Launch, directory, launch_with, project_root, refusal, refusal_with};

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

#[test]
fn a_guessed_launch_directory_that_is_not_a_project_opens_no_session() {
    let home = Path::new("/Users/someone");
    for dir in [Path::new("/"), home, Path::new("/Users")] {
        let launch = launch_with(None, Some(dir.to_path_buf()), Some(home));
        let Launch::Empty(words) = launch else {
            panic!("{} opened a session", dir.display());
        };
        assert!(words.contains("--worktree"), "{words}");
    }
    // A project still opens, so the refusal is about the directory and not about launching.
    let dir = tempfile::tempdir().unwrap();
    let Launch::Project(root) = launch_with(None, Some(dir.path().to_path_buf()), Some(home))
    else {
        panic!("a project opened no session");
    };
    assert_eq!(root, dir.path().canonicalize().unwrap());
}

#[test]
fn an_explicit_choice_is_never_refused_even_at_a_root_or_a_home() {
    let home = Path::new("/Users/someone");
    // `--worktree /` and a folder picked as home are decisions: each opens a session there.
    let Launch::Project(root) = launch_with(Some(PathBuf::from("/")), None, Some(home)) else {
        panic!("a root given by flag opened no session");
    };
    assert_eq!(root, Path::new("/"));
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let Launch::Project(named) = launch_with(Some(home.clone()), None, Some(&home)) else {
            panic!("a home given by flag opened no session");
        };
        assert_eq!(named, home.canonicalize().unwrap());
    }
    // The helper a picked folder goes through has the same rule, and so does `project_root`'s
    // explicit arm: readable and a directory is the whole test.
    assert_eq!(directory(PathBuf::from("/")).unwrap(), Path::new("/"));
    assert_eq!(
        project_root(Some(PathBuf::from("/"))).unwrap(),
        Path::new("/")
    );
}

#[test]
fn a_launch_directory_that_is_gone_opens_no_session_with_words() {
    let launch = launch_with(None, Some(PathBuf::from("/no/such/perch/dir")), None);
    let Launch::Empty(words) = launch else {
        panic!("a missing launch directory opened a session");
    };
    assert!(words.contains("not readable"), "{words}");
    // An explicit flag naming a missing directory is words too, never a window that refuses to
    // open: the person can pick one in it instead.
    assert!(matches!(
        launch_with(Some(PathBuf::from("/no/such/perch/dir")), None, None),
        Launch::Empty(_)
    ));
}
