//! Reading a file: line numbers, and the 64 KiB cap.

use perch_tools::ToolRegistry;
use serde_json::json;

use crate::support::root_of;

#[test]
fn read_caps_at_64_kib_and_numbers_lines() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("small.txt"), "alpha\nbeta\ngamma\n").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let out = registry
        .call("read", &json!({"path": "small.txt"}), &ctx)
        .unwrap();
    let lines: Vec<&str> = out.text.lines().collect();
    assert_eq!(lines[0], "     1\talpha");
    assert_eq!(lines[2], "     3\tgamma");

    let mut big = String::new();
    for i in 0..8_000 {
        big.push_str(&format!("line {i}\n"));
    }
    big.push_str("TAIL_MARKER\n");
    std::fs::write(root.path().join("big.txt"), &big).unwrap();
    let out = registry
        .call("read", &json!({"path": "big.txt"}), &ctx)
        .unwrap();
    assert!(
        !out.text.contains("TAIL_MARKER"),
        "read returned content past its 64 KiB cap"
    );
    assert!(
        out.text.contains("[truncated"),
        "output was {}",
        &out.text[..60]
    );
}
