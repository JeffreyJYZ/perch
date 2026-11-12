//! The command a dock tab runs: what an empty line means, how a line is split, and a command this
//! machine cannot run being refused by name.

use std::{path::Path, sync::mpsc::Receiver, time::Duration};

use perch_term::{Frame, ShellCommand, TermSession, shell::missing_program};

/// How long a test waits for the spawned command to print.
const TIMEOUT: Duration = Duration::from_secs(15);

#[test]
fn an_empty_line_is_this_machines_own_shell() {
    let expected = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
    assert_eq!(
        ShellCommand::parse("").program(),
        expected,
        "an empty setting is the shell a terminal already uses, not a command that runs nothing"
    );
    let blank = ShellCommand::parse("   ");
    assert_eq!(blank.program(), expected, "whitespace names nothing either");
    assert!(blank.args().is_empty());
    assert_eq!(missing_program(""), None, "the default is never refused");
}

#[test]
fn a_command_line_is_split_the_way_a_shell_reads_it() {
    let plain = ShellCommand::parse("/bin/zsh -l");
    assert_eq!(plain.program(), "/bin/zsh");
    assert_eq!(plain.args(), ["-l".to_string()]);

    let quoted = ShellCommand::parse("/bin/zsh -c 'echo hi  there'");
    assert_eq!(
        quoted.args(),
        ["-c".to_string(), "echo hi  there".to_string()]
    );

    let doubled = ShellCommand::parse("/bin/zsh --flag=\"a b\"");
    assert_eq!(doubled.args(), ["--flag=a b".to_string()]);
}

#[test]
fn a_command_this_machine_cannot_find_is_refused_by_name() {
    assert_eq!(missing_program("/bin/sh"), None, "/bin/sh is runnable");
    assert_eq!(
        missing_program("sh"),
        None,
        "a bare name is looked up on PATH"
    );
    assert_eq!(
        missing_program("perch-no-such-shell"),
        Some("perch-no-such-shell".to_string())
    );

    let refused = match TermSession::spawn_command(Path::new("/"), "perch-no-such-shell", 40, 12) {
        Ok(_) => panic!("a command nothing can run must not open a pty"),
        Err(error) => error,
    };
    assert!(
        refused.to_string().contains("perch-no-such-shell"),
        "the refusal names the command: {refused}"
    );
}

#[test]
fn the_saved_line_is_the_program_that_runs() {
    // A whole line, quoting and all: `-c` and its script are the arguments the line named.
    let spawned = TermSession::spawn_command(
        Path::new("/"),
        "/bin/sh -c 'echo perch-shell-line-42; sleep 30'",
        60,
        8,
    )
    .expect("the saved line starts");
    let (_, frames) = spawned;
    assert!(
        printed(&frames, "perch-shell-line-42"),
        "the arguments the line carried are the ones the command ran"
    );
}

/// Reads frames until one holds `needle`, or [`TIMEOUT`] runs out.
fn printed(frames: &Receiver<Frame>, needle: &str) -> bool {
    let deadline = std::time::Instant::now() + TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let Ok(frame) = frames.recv_timeout(left) else {
            return false;
        };
        let text: String = frame
            .rows
            .iter()
            .map(|row| row.cells.iter().map(|cell| cell.text.as_str()).collect())
            .collect::<Vec<String>>()
            .join("\n");
        if text.contains(needle) {
            return true;
        }
    }
}
