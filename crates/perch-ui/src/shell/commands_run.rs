//! What each registered command does to the window.
//!
//! One dispatch by name, beside the table in [`commands`](super::commands), so adding a command
//! is two edits in two neighbouring files. Every effect is real — a session opened, a title
//! written, a file saved — and a command whose precondition is missing says exactly what is
//! missing in the transcript rather than doing nothing.
//!
//! The session-shaped effects (a new session, a rename, an export, a delete) live in
//! [`commands_session`](super::commands_session); this file keeps the dispatch and the rest.

use std::path::Path;

use super::commands::{Command, find, help_lines, parse};
use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// Runs the slash command `text` names, and reports whether `text` was a command at all.
    ///
    /// Text that does not start with `/` is a prompt and answers false. A slash that names
    /// nothing, or a command that cannot run here, is said in the transcript in words.
    pub fn run_command(&mut self, text: &str) -> bool {
        let Some((name, arg)) = parse(text) else {
            return false;
        };
        if name.is_empty() {
            self.say("type a command name after the slash; /help lists every one");
            return true;
        }
        let Some(command) = find(name) else {
            self.say(&format!("/{name} is not a command; /help lists every one"));
            return true;
        };
        // A command that takes nothing, given words, is an ordinary prompt: only the bare word
        // is the control, so `/compact this please` reaches the model exactly as it was typed.
        if command.arg.is_none() && !arg.is_empty() {
            return false;
        }
        match self.command(command, arg) {
            Ok(Some(words)) => self.say(&words),
            Ok(None) => {}
            Err(words) => self.say(&words),
        }
        true
    }

    /// One command's effect. `Err` is the sentence the transcript says instead.
    fn command(&mut self, command: &Command, arg: &str) -> Result<Option<String>, String> {
        match command.name {
            "new" => self.command_new(),
            "fork" => self.command_fork(),
            "rename" => self.command_rename(arg),
            "model" => self.command_model(arg),
            "compact" => self.command_compact(),
            "theme" => self.command_theme(arg),
            "themes" => self.command_themes(),
            "terminal" => self
                .open_terminal()
                .map(|index| Some(format!("terminal: tab {}", index + 1))),
            "editor" => self.command_editor(),
            "sessions" => self.command_sessions(),
            "skills" => self.command_skills(),
            "skill" => self.command_skill(arg),
            "diff" => self.command_diff(arg),
            "export" => self.command_export(),
            "help" => {
                for line in help_lines() {
                    self.say(&line);
                }
                Ok(None)
            }
            "quit" => {
                self.quit = true;
                Ok(None)
            }
            "clean" => self.command_clean(),
            other => Err(format!("/{other} is not wired to anything")),
        }
    }

    /// `/model [id]`: the list, or the id given.
    fn command_model(&mut self, arg: &str) -> Result<Option<String>, String> {
        if arg.trim().is_empty() {
            if self.live.is_none() {
                return Err("no app is running, so no model list was opened".to_string());
            }
            self.open_model_panel();
            return Ok(Some("model: pick one from the list".to_string()));
        }
        let id = arg.trim().to_string();
        let Some(live) = self.live.as_mut() else {
            return Err("no app is running, so the model was not changed".to_string());
        };
        live.choose_model(id.clone())?;
        Ok(Some(format!("model: {id}")))
    }

    /// `/compact`: the conversation, through the app's own queue.
    fn command_compact(&mut self) -> Result<Option<String>, String> {
        let Some(live) = self.live.as_mut() else {
            return Err("no session is open, so nothing was compacted".to_string());
        };
        live.compact();
        Ok(None)
    }

    /// `/theme [name]`: the kit the next frame is drawn with, saved when there is an app.
    ///
    /// A name is checked against the library before anything switches, so an unknown one is
    /// refused in words that name what exists and the window keeps the theme it was drawing. A
    /// bare `/theme` is the toggle it has always been.
    fn command_theme(&mut self, arg: &str) -> Result<Option<String>, String> {
        let library = crate::theme::Library::discover();
        let word = match arg.trim() {
            "" if self.dark() => "light".to_string(),
            "" => "dark".to_string(),
            other => {
                library.choose(other)?;
                other.to_string()
            }
        };
        let size = self.appearance().font_size;
        self.set_appearance(crate::theme::Appearance::resolve(&library, &word, size));
        let saved = self
            .live
            .as_mut()
            .map_or(Err("no app is running".to_string()), |live| {
                live.set_theme(&word)
            });
        Ok(Some(match saved {
            Ok(()) => format!("theme: {word}"),
            // The window did switch; only the file that a restart reads did not change.
            Err(words) => format!("theme: {word} for this window only ({words})"),
        }))
    }

    /// `/themes`: every theme this machine has, the drawn one marked, and why a file that will not
    /// load is still listed.
    ///
    /// One line per name, through the transcript, because a failure's reason is a sentence and a
    /// comma-joined list hides which name it belongs to.
    fn command_themes(&mut self) -> Result<Option<String>, String> {
        let active = self.appearance().theme;
        for line in crate::theme::Library::discover().listing(&active) {
            self.say(&line);
        }
        Ok(None)
    }

    /// `/editor`: the selected file, handed to Zed.
    fn command_editor(&mut self) -> Result<Option<String>, String> {
        let Some(files) = self.files.as_ref() else {
            return Err("no file is open, so nothing was handed to Zed".to_string());
        };
        let Some(path) = files.selected_path().map(Path::to_path_buf) else {
            return Err("no file is open, so nothing was handed to Zed".to_string());
        };
        if files.editor().is_none() {
            return Err("no editor is attached, so nothing was handed to Zed".to_string());
        }
        if !self.open_in_zed() {
            return Err(format!("Zed refused {}", path.display()));
        }
        Ok(Some(format!("opened in Zed: {}", path.display())))
    }

    /// `/sessions`: the keyboard to the session list, where Up and Down move and Escape returns.
    fn command_sessions(&mut self) -> Result<Option<String>, String> {
        if self.sessions.is_empty() {
            return Err("no session is open, so there is no list to focus".to_string());
        }
        self.sessions_focused = true;
        self.focus_sessions = true;
        Ok(Some(
            "sessions: up and down move, Escape returns to the prompt".to_string(),
        ))
    }

    /// Writes one line into the transcript the window draws and the app keeps.
    pub(super) fn say(&mut self, words: &str) {
        if let Some(transcript) = self.live.as_mut().and_then(Live::transcript_mut) {
            transcript.note(words);
        }
        self.transcript.note(words);
    }
}
