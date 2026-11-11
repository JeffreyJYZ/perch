//! The porcelain-v2 headers, parsed without git.

use super::*;

#[test]
fn a_branch_with_an_upstream_reports_its_drift() {
    let text = "\
# branch.oid 0f2a9c1d4b5e6f708192a3b4c5d6e7f809a1b2c3
# branch.head feat/files
# branch.upstream origin/feat/files
# branch.ab +3 -1
1 .M N... 100644 100644 100644 0f2a9c1d4b5e6f708192a3b4c5d6e7f809a1b2c3 0f2a9c1d4b5e6f708192a3b4c5d6e7f809a1b2c3 src/lib.rs
";
    let branch = parse_branch(text).expect("a head line is a branch");
    assert_eq!(branch.name, "feat/files");
    assert_eq!(branch.upstream.as_deref(), Some("origin/feat/files"));
    assert_eq!(branch.ahead, 3);
    assert_eq!(branch.behind, 1);
    assert!(!branch.detached);
}

#[test]
fn a_detached_head_is_named_head_and_flagged() {
    let text = "# branch.oid 0f2a9c1d4b5e6f708192a3b4c5d6e7f809a1b2c3\n# branch.head (detached)\n";
    let branch = parse_branch(text).expect("a detached head is still a branch");
    assert_eq!(branch.name, "HEAD");
    assert!(branch.detached);
    assert_eq!(branch.upstream, None);
    assert_eq!((branch.ahead, branch.behind), (0, 0));
}

#[test]
fn no_upstream_means_no_drift() {
    let text = "# branch.oid (initial)\n# branch.head main\n";
    let branch = parse_branch(text).expect("an unborn branch is still a branch");
    assert_eq!(branch.name, "main");
    assert_eq!(branch.upstream, None);
    assert_eq!(branch.ahead, 0);
    assert_eq!(branch.behind, 0);
    assert!(!branch.detached);
}

#[test]
fn no_head_line_is_no_branch() {
    assert_eq!(parse_branch(""), None);
    assert_eq!(
        parse_branch("1 .M N... 100644 100644 100644 abc abc src/lib.rs\n"),
        None
    );
}
