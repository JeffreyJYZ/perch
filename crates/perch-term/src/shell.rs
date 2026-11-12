//! The command line a terminal tab runs: what an empty line means, and how a line is split.
//!
//! A saved line is a command, not a path: quoting is the person's, so `sh -l` and
//! `sh -c 'echo hi'` mean what a shell would make of them. The machine's own shell is the
//! default, which is why an empty line is not a command that runs nothing.

use std::path::Path;

use portable_pty::CommandBuilder;

/// A shell command line, split into the program and the arguments it names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellCommand {
    program: String,
    args: Vec<String>,
}

impl ShellCommand {
    /// `line` as a program and its arguments. A line that names none runs this machine's shell.
    #[must_use]
    pub fn parse(line: &str) -> Self {
        let mut words = split(line);
        let program = if words.is_empty() {
            machine_default()
        } else {
            words.remove(0)
        };
        Self {
            program,
            args: words,
        }
    }

    /// The program the line starts.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The arguments it starts with.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// The pty command this line describes, ready for a shell.
    pub(crate) fn builder(&self) -> CommandBuilder {
        let mut command = CommandBuilder::new(&self.program);
        for arg in &self.args {
            command.arg(arg);
        }
        command
    }
}

/// The program `line` names, when this machine has no such command.
///
/// Both the settings pane and the spawn ask this before anything starts: a line nothing can run is
/// refused by name, never a tab that dies as it opens. A line that names nothing at all is the
/// default, not a refusal.
#[must_use]
pub fn missing_program(line: &str) -> Option<String> {
    let program = ShellCommand::parse(line).program;
    (!runnable(&program)).then_some(program)
}

/// The shell a line that names none runs: the one the person's terminal already uses.
fn machine_default() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned())
}

/// Whether `program` is a file this machine can run, by its path or on `PATH`.
fn runnable(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }
    if program.contains('/') {
        return executable(Path::new(program));
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| executable(&dir.join(program))))
}

/// Whether `path` is a file with an execute bit set.
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Splits `line` into words the way a shell's quoting says: whitespace separates, a quote keeps a
/// word whole, and a backslash keeps the character after it.
fn split(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut held = false;
    let mut chars = line.chars();
    while let Some(character) = chars.next() {
        match (quote, character) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), '\\') => {
                if let Some(escaped) = chars.next() {
                    word.push(escaped);
                }
            }
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(character);
                held = true;
            }
            (None, '\\') => {
                if let Some(escaped) = chars.next() {
                    word.push(escaped);
                    held = true;
                }
            }
            (None, c) if c.is_whitespace() => {
                if held || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                    held = false;
                }
            }
            (None, c) => {
                word.push(c);
                held = true;
            }
        }
    }
    if held || !word.is_empty() {
        words.push(word);
    }
    words
}
