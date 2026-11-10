//! The regions: a tab switch moves the session, and the dump names what the layout promises.

use std::process::Command;

use perch_ui::Shell;

use crate::harness::meta;

#[test]
fn selecting_a_tab_changes_the_active_session() {
    let mut shell = Shell::idle(vec![meta("a"), meta("b")], 0);
    assert_eq!(shell.active(), 0);
    shell.select(1);
    assert_eq!(shell.active(), 1);
    assert_eq!(shell.tab_titles(), vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn tree_dump_names_every_region() {
    let out = Command::new(env!("CARGO_BIN_EXE_perch"))
        .args(["--tree", "--offline"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for region in ["sessions", "transcript", "terminal", "files"] {
        assert!(
            text.contains(region),
            "accessibility tree is missing {region}:\n{text}"
        );
    }
}
