//! Bash: the output cap, the working directory, a failing command and the timeout.

use std::path::Path;
use std::time::{Duration, Instant};

use perch_tools::{BASH_TIMEOUT, Bash, Error, Tool, ToolRegistry};
use serde_json::json;

use crate::support::{canonical, root_of};

#[test]
fn bash_output_is_truncated_to_the_stated_budget() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let out = ToolRegistry::builtin()
        .call("bash", &json!({"command": "yes x | head -c 200000"}), &ctx)
        .unwrap();

    assert!(out.text.len() <= 16 * 1024, "len was {}", out.text.len());
    assert!(
        out.text.contains("[truncated"),
        "output was {}",
        &out.text[..80]
    );
}

#[test]
fn bash_runs_in_the_root_and_reports_a_failing_command() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let out = registry
        .call("bash", &json!({"command": "pwd -P"}), &ctx)
        .unwrap();
    assert_eq!(
        canonical(Path::new(out.text.trim())),
        canonical(root.path())
    );
    assert_eq!(BASH_TIMEOUT, Duration::from_secs(30));

    let failed = registry
        .call("bash", &json!({"command": "echo boom >&2; exit 3"}), &ctx)
        .unwrap();
    assert!(failed.text.contains("boom"), "output was {}", failed.text);
    assert!(failed.text.contains("exit 3"), "output was {}", failed.text);
}

#[test]
fn bash_is_killed_and_reaped_on_timeout() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let bash = Bash::with_timeout(Duration::from_millis(300));

    let started = Instant::now();
    let out = bash.call(&json!({"command": "echo $$ > pid.txt; exec sleep 5"}), &ctx);
    let elapsed = started.elapsed();

    let Err(Error::Timeout { .. }) = &out else {
        panic!("expected a timeout, got {out:?}");
    };
    assert!(
        elapsed < Duration::from_secs(3),
        "timeout took {elapsed:?}, so the child was not killed"
    );

    let pid = std::fs::read_to_string(root.path().join("pid.txt"))
        .unwrap()
        .trim()
        .to_string();
    let ps = std::process::Command::new("ps")
        .args(["-p", &pid])
        .output()
        .unwrap();
    let listing = String::from_utf8_lossy(&ps.stdout);
    assert!(
        !listing.contains(&pid),
        "pid {pid} still exists after the timeout, so it was not reaped: {listing}"
    );
}
