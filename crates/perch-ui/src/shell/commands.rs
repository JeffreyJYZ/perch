//! The one list of commands the composer and the palette are built from.
//!
//! A command is a name, one line saying what it does, and whether it takes an argument. The
//! effects live in [`commands_run`](super::commands_run): this file is the table every surface
//! reads, so `/help`, the autocomplete list and the palette can never disagree about what exists.

/// One command a person can type, with a leading `/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// The name without its `/`: `rename`, not `/rename`.
    pub name: &'static str,
    /// One line, in the words the help list and the palette show.
    pub about: &'static str,
    /// The argument's placeholder, or `None` when the command takes no argument.
    pub arg: Option<&'static str>,
}

impl Command {
    /// A command that takes nothing: only the bare word is the control.
    const fn bare(name: &'static str, about: &'static str) -> Command {
        Command {
            name,
            about,
            arg: None,
        }
    }

    /// A command whose typed form carries an argument, named by its placeholder.
    const fn taking(name: &'static str, about: &'static str, arg: &'static str) -> Command {
        Command {
            name,
            about,
            arg: Some(arg),
        }
    }

    /// The name as it is typed: `/rename <title>`.
    #[must_use]
    pub fn label(&self) -> String {
        match self.arg {
            Some(arg) => format!("/{} {arg}", self.name),
            None => format!("/{}", self.name),
        }
    }

    /// The line `/help` writes for this command.
    #[must_use]
    pub fn help_line(&self) -> String {
        format!("{} — {}", self.label(), self.about)
    }
}

/// Every command, in the order `/help` and the autocomplete list show them.
pub const COMMANDS: &[Command] = &[
    Command::bare("new", "start a session on this worktree"),
    Command::bare("fork", "copy this conversation into a new session"),
    Command::taking("rename", "name this session", "<title>"),
    Command::taking("model", "open the model list, or set one by id", "[id]"),
    Command::bare("compact", "drop the older exchange from the conversation"),
    Command::taking("theme", "switch to a theme; /themes lists them", "[name]"),
    Command::bare("themes", "list the themes on this machine"),
    Command::bare("terminal", "open a new terminal tab"),
    Command::bare("editor", "hand the open file to Zed"),
    Command::bare("sessions", "focus the session list"),
    Command::bare("skills", "list the skills this session can load"),
    Command::taking("skill", "load a skill's body into the prompt", "<name>"),
    Command::taking(
        "diff",
        "show the files the last turn changed, and their diffs",
        "[path]",
    ),
    Command::bare("export", "write this transcript to a file"),
    Command::bare("help", "list every command"),
    Command::bare("quit", "close the window"),
    Command::bare("clean", "empty this session's transcript"),
];

/// The command named `name`, without its slash.
#[must_use]
pub fn find(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.name == name)
}

/// What a composer's text asks for: the name and its argument, when it starts with a slash.
///
/// `None` is ordinary text — a prompt, or a path — so the caller sends it to the model rather
/// than refusing it as an unknown command.
#[must_use]
pub fn parse(text: &str) -> Option<(&str, &str)> {
    let rest = text.trim().strip_prefix('/')?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default();
    let arg = parts.next().unwrap_or_default().trim();
    Some((name, arg))
}

/// The commands whose typed form starts with `filter`, the text after the slash.
///
/// An empty filter is all of them: typing `/` is how the list is asked for. A filter with a
/// space in it matches nothing, because the argument is not a command name.
#[must_use]
pub fn matching(filter: &str) -> Vec<&'static Command> {
    let needle = filter.trim().to_lowercase();
    COMMANDS
        .iter()
        .filter(|command| needle.is_empty() || command.name.starts_with(&needle))
        .collect()
}

/// The filter part of a composer text that is being typed as a command: everything after `/`
/// up to the first space. `None` when the text is not a bare command being typed.
#[must_use]
pub fn filter_of(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let rest = trimmed.strip_prefix('/')?;
    if rest.contains(char::is_whitespace) || rest.contains('\n') {
        return None;
    }
    Some(rest)
}

/// Every command as a help line, one per registered command.
#[must_use]
pub fn help_lines() -> Vec<String> {
    COMMANDS.iter().map(Command::help_line).collect()
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, filter_of, find, help_lines, matching, parse};

    #[test]
    fn a_slash_names_a_command_and_its_argument() {
        assert_eq!(parse("/rename my session"), Some(("rename", "my session")));
        assert_eq!(parse("  /help  "), Some(("help", "")));
        assert_eq!(parse("what is /compact"), None, "ordinary text is a prompt");
        assert_eq!(parse("/"), Some(("", "")));
    }

    #[test]
    fn the_filter_narrows_to_the_names_that_start_with_it() {
        let names = |filter: &str| -> Vec<&'static str> {
            matching(filter)
                .into_iter()
                .map(|command| command.name)
                .collect()
        };
        assert_eq!(names("").len(), COMMANDS.len());
        assert_eq!(names("re"), ["rename"]);
        assert_eq!(names("ca"), Vec::<&str>::new(), "compact starts with co");
        assert_eq!(names("co"), ["compact"]);
    }

    #[test]
    fn the_filter_stops_at_the_argument() {
        assert_eq!(filter_of("/ren"), Some("ren"));
        assert_eq!(filter_of("/rename my session"), None);
        assert_eq!(filter_of("plain"), None);
    }

    #[test]
    fn every_registered_name_is_found_and_has_a_help_line() {
        let lines = help_lines();
        assert_eq!(lines.len(), COMMANDS.len());
        for (command, line) in COMMANDS.iter().zip(lines) {
            assert_eq!(find(command.name), Some(command));
            assert!(line.starts_with(&format!("/{}", command.name)), "{line}");
            assert!(!command.about.is_empty());
        }
        assert_eq!(find("nope"), None);
    }
}
