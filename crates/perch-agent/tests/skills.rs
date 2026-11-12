//! What skill discovery finds on disk, and what it passes over without failing.

use std::path::{Path, PathBuf};

use perch_agent::skills::{MAX_BODY_BYTES, discover_in, load_in};

/// Writes one file, making the directories it sits in.
fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The `SKILL.md` a skill named `name` keeps under `root`'s own project directory.
fn skill_file(root: &Path, name: &str) -> PathBuf {
    root.join(".perch/skills").join(name).join("SKILL.md")
}

/// A machine-wide skill named `name`.
fn machine_file(home: &Path, name: &str) -> PathBuf {
    home.join(".config/perch/skills")
        .join(name)
        .join("SKILL.md")
}

/// The names `discover_in` lists, in order.
fn names(skills: &[perch_agent::Skill]) -> Vec<String> {
    skills.iter().map(|skill| skill.name.clone()).collect()
}

#[test]
fn both_roots_are_read_and_the_nearer_file_wins_a_duplicate_name() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    write(
        &machine_file(home.path(), "rusty"),
        "---\nname: rusty\ndescription: machine rules\n---\nmachine body",
    );
    write(
        &skill_file(project.path(), "rusty"),
        "---\nname: rusty\ndescription: project rules\n---\nproject body",
    );
    write(
        &skill_file(project.path(), "ship"),
        "---\nname: ship\ndescription: how a release is cut\n---\nship body",
    );

    let found = discover_in(Some(project.path()), Some(home.path()));
    assert_eq!(
        names(&found.skills),
        ["rusty", "ship"],
        "both roots, one row"
    );
    let rusty = found
        .skills
        .iter()
        .find(|skill| skill.name == "rusty")
        .expect("rusty is listed");
    assert_eq!(rusty.body, "project body", "the nearer body wins");
    assert_eq!(rusty.description.as_deref(), Some("project rules"));
}

#[test]
fn a_file_without_front_matter_is_skipped_and_counted() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    write(
        &skill_file(project.path(), "notes"),
        "just a note, no block",
    );
    write(
        &skill_file(project.path(), "good"),
        "---\nname: good\n---\ngood body",
    );

    let found = discover_in(Some(project.path()), Some(home.path()));
    assert_eq!(names(&found.skills), ["good"]);
    assert_eq!(found.skipped, 1, "the file with no front matter is counted");
}

#[test]
fn a_missing_name_is_the_directory_and_a_missing_description_is_none() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    write(&skill_file(project.path(), "quiet"), "---\n---\nquiet body");

    let skill =
        load_in(Some(project.path()), Some(home.path()), "quiet").expect("named by its dir");
    assert_eq!(skill.description, None);
    assert_eq!(skill.body, "quiet body");
}

#[test]
fn a_body_over_the_cap_is_cut_and_the_cut_is_named() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let body = "x".repeat(100 * 1024);
    write(
        &skill_file(project.path(), "big"),
        &format!("---\nname: big\n---\n{body}"),
    );

    let skill = load_in(Some(project.path()), Some(home.path()), "big").expect("big is found");
    assert_eq!(
        &skill.body[..MAX_BODY_BYTES],
        "x".repeat(MAX_BODY_BYTES),
        "exactly the cap was kept"
    );
    let note = &skill.body[MAX_BODY_BYTES..];
    assert!(
        note.contains(&format!("only the first {MAX_BODY_BYTES} were read")),
        "{note}"
    );
    assert!(note.contains("102400"), "the real size is named: {note}");
    assert!(note.ends_with("The rest was cut.)"), "{note}");
}

#[test]
fn load_of_an_unknown_name_is_none() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    write(
        &skill_file(project.path(), "good"),
        "---\nname: good\n---\nbody",
    );

    assert!(load_in(Some(project.path()), Some(home.path()), "nope").is_none());
    assert!(load_in(Some(project.path()), Some(home.path()), "good").is_some());
}
