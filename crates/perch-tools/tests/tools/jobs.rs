//! Background jobs: returns at once, output readable after exit, kill by group, and the cap.

use std::time::{Duration, Instant};

use perch_tools::{JOB_MAX_CONCURRENT, ToolCtx, ToolRegistry};
use serde_json::json;

use crate::support::root_of;

/// The id and pid a `background: true` bash call answered with.
fn start(registry: &ToolRegistry, ctx: &ToolCtx, command: &str) -> (String, u32) {
    let out = registry
        .call(
            "bash",
            &json!({"command": command, "background": true}),
            ctx,
        )
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(out.text.trim())
        .unwrap_or_else(|error| panic!("bash did not answer with JSON ({error}): {}", out.text));
    let id = value["job"].as_str().unwrap().to_string();
    let pid = value["pid"].as_u64().unwrap() as u32;
    assert_eq!(value["command"], command);
    (id, pid)
}

fn jobs(registry: &ToolRegistry, ctx: &ToolCtx, args: serde_json::Value) -> String {
    registry.call("jobs", &args, ctx).unwrap().text
}

#[test]
fn a_background_command_returns_at_once_and_its_output_is_readable_when_it_exits() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let started = Instant::now();
    let (id, pid) = start(&registry, &ctx, "echo first; sleep 1; echo done");
    let took = started.elapsed();
    assert!(
        took < Duration::from_millis(500),
        "bash waited for its child instead of returning: {took:?}"
    );
    assert!(pid > 0, "no pid was reported");

    let list = jobs(&registry, &ctx, json!({"op": "list"}));
    assert!(list.contains(&id), "the job is missing from `list`: {list}");
    assert!(list.contains("running"), "{list}");
    assert!(list.contains(&pid.to_string()), "{list}");
    assert!(list.contains("log"), "{list}");

    let mut list = String::new();
    for _ in 0..100 {
        list = jobs(&registry, &ctx, json!({"op": "list"}));
        if list.contains("exited") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        list.contains("exited"),
        "the job never reported exiting: {list}"
    );
    assert!(
        !list.contains("running"),
        "a finished job is still reported as running: {list}"
    );

    let log = jobs(&registry, &ctx, json!({"op": "read", "id": id}));
    assert!(log.contains("first"), "read missed the early output: {log}");
    assert!(log.contains("done"), "read missed the later output: {log}");
}

#[test]
fn killing_a_job_kills_its_process_group_and_is_reported_as_killed() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();
    let (id, pid) = start(&registry, &ctx, "sleep 30 & echo $! > child.pid; wait");
    let child_pid = wait_for_pid(&root.path().join("child.pid"));

    let killed = jobs(&registry, &ctx, json!({"op": "kill", "id": id}));
    assert!(killed.contains(&id), "{killed}");
    assert!(killed.contains("killed"), "{killed}");
    assert!(
        killed.contains(&pid.to_string()),
        "the report does not say what it killed: {killed}"
    );

    let list = jobs(&registry, &ctx, json!({"op": "list"}));
    assert!(list.contains("killed"), "{list}");
    assert!(
        !list.contains("running"),
        "a killed job is still reported as running: {list}"
    );
    assert!(
        !list.contains("exited"),
        "a killed job must not be reported as a clean exit: {list}"
    );

    for gone in [pid, child_pid] {
        assert!(!alive(gone), "pid {gone} survived the kill: {gone}");
    }

    let again = jobs(&registry, &ctx, json!({"op": "kill", "id": id}));
    assert!(
        again.contains("already"),
        "killing a job twice must not be an error: {again}"
    );
}

/// The child of the job's own shell, which only the process group can reach.
fn wait_for_pid(path: &std::path::Path) -> u32 {
    for _ in 0..100 {
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(pid) = text.trim().parse::<u32>()
        {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the job never wrote {}", path.display());
}

fn alive(pid: u32) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
}

#[test]
fn the_seventeenth_job_is_refused_and_the_refusal_names_the_cap() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let mut ids = Vec::new();
    for _ in 0..JOB_MAX_CONCURRENT {
        let (id, _) = start(&registry, &ctx, "sleep 20");
        ids.push(id);
    }

    let refused = registry
        .call(
            "bash",
            &json!({"command": "sleep 20", "background": true}),
            &ctx,
        )
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains(&JOB_MAX_CONCURRENT.to_string()),
        "the refusal does not name the cap of {JOB_MAX_CONCURRENT}: {refused}"
    );

    for id in ids {
        let _ = registry.call("jobs", &json!({"op": "kill", "id": id}), &ctx);
    }
}
