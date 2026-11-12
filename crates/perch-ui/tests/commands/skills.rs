//! `/skills` and `/skill`: what the list says, and where a body lands when it is asked for.

use std::path::Path;

use perch_ui::Shell;
use tempfile::TempDir;

use crate::harness::{Fake, notes};

/// Tests must not read the machine they run on.
///
/// The machine's own skills live under the home directory, so a real `SKILL.md` would change what
/// `/skills` says here. This points the home at an empty directory once per process; `PERCH_HOME`
/// is the seam `perch_agent::skills` documents.
fn hermetic() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = std::env::temp_dir().join("perch-ui-commands-home");
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::create_dir_all(&home);
        // Safety: it is set once per process, before any command reads the home, under test only.
        unsafe { std::env::set_var("PERCH_HOME", &home) };
    });
}

/// A shell over one session on a throwaway worktree, with the home isolated.
fn shell() -> (TempDir, Shell<Fake>) {
    hermetic();
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::one("one", dir.path());
    let shell = Shell::new(fake.metas.clone(), 0).with_live(fake);
    (dir, shell)
}

/// Writes `<worktree>/.perch/skills/<name>/SKILL.md`.
fn plant(worktree: &Path, name: &str, front: &str, body: &str) {
    let dir = worktree.join(".perch/skills").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), format!("{front}\n{body}")).unwrap();
}

/// Types `text` and presses Enter, as the composer does.
fn run(shell: &mut Shell<Fake>, text: &str) {
    shell.set_prompt(text);
    assert!(shell.submit(), "{text} is a command and is taken");
}

#[test]
fn skills_on_an_empty_set_says_so_in_words() {
    let (_dir, mut shell) = shell();
    run(&mut shell, "/skills");
    let said = notes(&shell).join("\n");
    assert!(said.contains("no skills"), "{said}");
    assert!(
        said.contains("SKILL.md"),
        "it says where one would go: {said}"
    );
}

#[test]
fn skills_lists_name_and_description_as_one_row() {
    let (dir, mut shell) = shell();
    plant(
        dir.path(),
        "rusty",
        "---\nname: rusty\ndescription: how Rust is written here\n---",
        "use clippy",
    );
    run(&mut shell, "/skills");
    assert_eq!(
        notes(&shell),
        ["rusty — how Rust is written here".to_string()]
    );
}

#[test]
fn skill_lands_the_body_in_the_composer_and_sends_nothing() {
    let (dir, mut shell) = shell();
    plant(
        dir.path(),
        "rusty",
        "---\nname: rusty\ndescription: how Rust is written here\n---",
        "use clippy before a commit",
    );
    run(&mut shell, "/skill rusty");
    assert_eq!(
        shell.prompt_text(),
        "use clippy before a commit",
        "the body is the prompt, waiting to be edited"
    );
    assert!(
        shell.live().unwrap().prompts.is_empty(),
        "nothing was sent to the model"
    );
}

#[test]
fn a_skill_name_that_matches_nothing_is_refused_and_points_at_the_list() {
    let (_dir, mut shell) = shell();
    run(&mut shell, "/skill nope");
    assert!(
        notes(&shell)
            .iter()
            .any(|note| note == "no skill named nope; /skills lists them"),
        "{:?}",
        notes(&shell)
    );
    assert!(shell.prompt_text().is_empty(), "the box was left alone");
}

#[test]
fn a_bare_skill_asks_for_a_name() {
    let (_dir, mut shell) = shell();
    run(&mut shell, "/skill");
    assert!(
        notes(&shell)
            .iter()
            .any(|note| note == "skill needs a name: /skill <name> — /skills lists them"),
        "{:?}",
        notes(&shell)
    );
}
