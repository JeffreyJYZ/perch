//! Integration tests for the tool registry: schemas, confinement, approvals and limits.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use perch_tools::{
    BASH_TIMEOUT, Bash, Error, Grep, Preview, Result, Tool, ToolCtx, ToolOutput, ToolRegistry,
    ToolSpec, confine,
};
use serde_json::{Value, json};
use tempfile::TempDir;

/// A tiny tool defined only here, proving registration is dynamic rather than a fixed list.
struct Echo;

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Echo the given text back.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let text = args["text"].as_str().unwrap_or_default().to_string();
        Ok(ToolOutput {
            text,
            preview: None,
        })
    }
}

fn root_of(dir: &TempDir) -> ToolCtx {
    ToolCtx {
        root: dir.path().to_path_buf(),
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[test]
fn every_builtin_tool_is_registered_with_a_schema() {
    let specs = ToolRegistry::builtin().specs();
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["read", "write", "edit", "bash", "list", "grep"]);

    for spec in &specs {
        assert!(
            !spec.description.is_empty(),
            "{} has no description",
            spec.name
        );
        let schema = &spec.parameters;
        assert_eq!(
            schema["type"], "object",
            "{} must declare an object schema, got {schema}",
            spec.name
        );
        assert!(
            schema["properties"].is_object(),
            "{} must declare properties, got {schema}",
            spec.name
        );
    }
}

#[test]
fn a_registered_tool_appears_in_the_specs_and_is_callable() {
    let mut registry = ToolRegistry::builtin();
    registry.register(Arc::new(Echo));

    assert!(registry.specs().iter().any(|s| s.name == "echo"));

    let ctx = ToolCtx {
        root: std::env::temp_dir(),
    };
    assert_eq!(
        registry
            .call("echo", &json!({"text": "hi"}), &ctx)
            .unwrap()
            .text,
        "hi"
    );
    assert!(matches!(
        registry.call("missing", &json!({}), &ctx),
        Err(Error::UnknownTool(name)) if name == "missing"
    ));
}

#[test]
fn parent_traversal_and_symlink_escape_are_refused() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ok.txt"), "x").unwrap();

    assert!(confine(root.path(), Path::new("../etc/passwd")).is_err());
    assert!(confine(root.path(), Path::new("ok.txt")).is_ok());
    assert!(confine(root.path(), Path::new("nested/new.txt")).is_ok());

    std::os::unix::fs::symlink("/etc", root.path().join("out")).unwrap();
    assert!(confine(root.path(), Path::new("out/hosts")).is_err());
}

#[test]
fn edit_returns_a_diff_preview_and_write_refuses_outside_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();
    let ctx = root_of(&root);

    let out = ToolRegistry::builtin()
        .call(
            "edit",
            &json!({"path": "a.rs", "old": "a", "new": "b"}),
            &ctx,
        )
        .unwrap();
    assert!(matches!(out.preview, Some(Preview::Diff(_))));
    assert_eq!(
        std::fs::read_to_string(root.path().join("a.rs")).unwrap(),
        "fn b() {}\n"
    );

    assert!(
        ToolRegistry::builtin()
            .call("write", &json!({"path": "../x", "text": "y"}), &ctx)
            .is_err()
    );
}

#[test]
fn edit_refuses_old_text_that_is_absent_or_ambiguous() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.rs"), "let a = 1;\nlet a = 2;\n").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let ambiguous = registry.call(
        "edit",
        &json!({"path": "a.rs", "old": "let a", "new": "let b"}),
        &ctx,
    );
    assert!(ambiguous.is_err(), "old text twice must be refused");
    let absent = registry.call(
        "edit",
        &json!({"path": "a.rs", "old": "nowhere", "new": "x"}),
        &ctx,
    );
    assert!(absent.is_err(), "absent old text must be refused");
    assert_eq!(
        std::fs::read_to_string(root.path().join("a.rs")).unwrap(),
        "let a = 1;\nlet a = 2;\n",
        "a refused edit must not touch the file"
    );
}

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

#[test]
fn grep_skips_binary_files_and_lists_matches_relative_to_the_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("text.txt"), "needle here\nnothing\n").unwrap();
    std::fs::write(root.path().join("bin.dat"), b"needle\0binary\n").unwrap();
    let ctx = root_of(&root);

    let out = ToolRegistry::builtin()
        .call("grep", &json!({"pattern": "needle", "path": "."}), &ctx)
        .unwrap();
    assert!(
        out.text.contains("text.txt:1:needle here"),
        "output was {}",
        out.text
    );
    assert!(
        !out.text.contains("./text.txt"),
        "paths must be relative to the root without a `./` prefix, output was {}",
        out.text
    );
    assert!(
        !out.text.contains("bin.dat"),
        "grep must skip binary files, output was {}",
        out.text
    );
}

#[test]
fn grep_treats_the_pattern_as_a_regex() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let out = registry
        .call("grep", &json!({"pattern": "fn\\s+main"}), &ctx)
        .unwrap();
    assert!(
        out.text.contains("main.rs:1:fn main() {}"),
        "a regex pattern must match whitespace runs, output was {}",
        out.text
    );

    let invalid = registry.call("grep", &json!({"pattern": "("}), &ctx);
    assert!(
        invalid.is_err(),
        "an invalid regex must be an error, got {invalid:?}"
    );
}

#[test]
fn grep_respects_gitignore() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("text.txt"), "needle here\n").unwrap();
    std::fs::write(root.path().join("ignored.txt"), "needle ignored\n").unwrap();
    std::fs::write(root.path().join(".gitignore"), "ignored.txt\n").unwrap();
    let ctx = root_of(&root);

    let out = ToolRegistry::builtin()
        .call("grep", &json!({"pattern": "needle", "path": "."}), &ctx)
        .unwrap();
    assert!(
        out.text.contains("text.txt:1:needle here"),
        "output was {}",
        out.text
    );
    assert!(
        !out.text.contains("ignored.txt"),
        "an ignored file must not be searched, output was {}",
        out.text
    );
}

#[test]
fn grep_reports_a_missing_rg_rather_than_matching_nothing() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("text.txt"), "needle here\n").unwrap();
    let ctx = root_of(&root);
    let grep = Grep::with_program("/nonexistent/perch-rg-probe");

    let err = grep
        .call(&json!({"pattern": "needle"}), &ctx)
        .expect_err("a missing rg must be an error, not an empty result");
    assert!(
        err.to_string().contains("perch-rg-probe"),
        "error must name the program, was {err}"
    );
}

#[test]
fn list_shows_entries_inside_the_root_only() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("sub")).unwrap();
    std::fs::write(root.path().join("a.txt"), "x").unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let out = registry.call("list", &json!({"path": "."}), &ctx).unwrap();
    assert!(out.text.contains("a.txt"), "output was {}", out.text);
    assert!(out.text.contains("sub/"), "output was {}", out.text);

    assert!(
        registry
            .call("list", &json!({"path": "../"}), &ctx)
            .is_err()
    );
    assert!(
        registry
            .call("read", &json!({"path": "/etc/hosts"}), &ctx)
            .is_err()
    );
}

#[test]
fn approvals_are_required_only_for_mutating_tools() {
    let registry = ToolRegistry::builtin();
    for name in ["read", "list", "grep"] {
        assert!(
            !registry.needs_approval(name).unwrap(),
            "{name} needs approval"
        );
    }
    for name in ["write", "edit", "bash"] {
        assert!(
            registry.needs_approval(name).unwrap(),
            "{name} needs no approval"
        );
    }
    let mut registry = registry;
    registry.register(Arc::new(Echo));
    assert!(!registry.needs_approval("echo").unwrap());
    assert!(registry.needs_approval("missing").is_err());
}

#[test]
fn unregistering_removes_the_spec_and_the_call_path() {
    let mut registry = ToolRegistry::builtin();
    registry.register(Arc::new(Echo));
    assert!(registry.unregister("echo"));
    assert!(!registry.specs().iter().any(|s| s.name == "echo"));
    let ctx = ToolCtx {
        root: TempDir::new().unwrap().path().to_path_buf(),
    };
    assert!(registry.call("echo", &json!({"text": "hi"}), &ctx).is_err());
    assert!(
        !registry.unregister("echo"),
        "second removal reported a change"
    );
}
