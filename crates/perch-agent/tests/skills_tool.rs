//! The `skill` tool: the body it answers with, and the list it answers a wrong name with.

use std::path::{Path, PathBuf};

use perch_agent::SkillTool;
use perch_tools::{Tool, ToolCtx};
use serde_json::json;

/// Writes one file, making the directories it sits in.
fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The `SKILL.md` a skill named `name` keeps under `root`'s own project directory.
fn skill_file(root: &Path, name: &str) -> PathBuf {
    root.join(".perch/skills").join(name).join("SKILL.md")
}

/// A worktree carrying two skills, and an empty home beside it so nothing reads the machine.
fn planted() -> (tempfile::TempDir, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    write(
        &skill_file(project.path(), "rusty"),
        "---\nname: rusty\ndescription: how Rust is written here\n---\nuse clippy before a commit",
    );
    write(
        &skill_file(project.path(), "ship"),
        "---\nname: ship\ndescription: how a release is cut\n---\ntag, then watch the run",
    );
    (home, project)
}

/// The context a call is given: the worktree root.
fn ctx(root: &Path) -> ToolCtx {
    ToolCtx {
        root: root.to_path_buf(),
    }
}

#[test]
fn the_body_is_returned_with_its_name_and_path_on_the_first_line() {
    let (home, project) = planted();
    let tool = SkillTool::in_home(home.path());
    let out = tool
        .call(&json!({"name": "rusty"}), &ctx(project.path()))
        .expect("a read cannot fail");

    let first = out.text.lines().next().expect("a first line");
    let path = skill_file(project.path(), "rusty");
    assert!(first.contains("rusty"), "{first}");
    assert!(first.contains(&path.display().to_string()), "{first}");
    assert_eq!(
        out.text.strip_prefix(first).unwrap().trim_start(),
        "use clippy before a commit"
    );
}

#[test]
fn a_name_that_does_not_exist_answers_with_the_names_that_do() {
    let (home, project) = planted();
    let tool = SkillTool::in_home(home.path());
    let out = tool
        .call(&json!({"name": "rust"}), &ctx(project.path()))
        .expect("a wrong name is an answer, not an error");

    assert!(out.text.contains("no skill named rust"), "{}", out.text);
    assert!(out.text.contains("available: rusty, ship"), "{}", out.text);
}

#[test]
fn a_machine_with_no_skills_says_so_rather_than_listing_nothing() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let tool = SkillTool::in_home(home.path());
    let out = tool
        .call(&json!({"name": "rusty"}), &ctx(project.path()))
        .expect("a wrong name is an answer, not an error");

    assert!(out.text.contains("none are installed"), "{}", out.text);
    assert!(out.text.contains("SKILL.md"), "{}", out.text);
}

#[test]
fn the_tool_loads_instructions_and_asks_for_no_approval() {
    let tool = SkillTool::default();
    assert!(!tool.needs_approval(), "reading a skill changes nothing");
    let spec = tool.spec();
    assert_eq!(spec.name, "skill");
    assert!(
        spec.description.contains("instructions, never code"),
        "{}",
        spec.description
    );
    assert!(
        spec.description
            .contains("follow it for the rest of the task")
    );
    assert_eq!(spec.parameters["required"], json!(["name"]));
}
