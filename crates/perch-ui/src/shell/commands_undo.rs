//! `/undo`: the files of the last turn, back as they were when it began.
//!
//! The app owns the snapshot — it is captured at the start of every turn — so the shell asks it
//! and says what came back: one note per path, then one line about the conversation, which a file
//! restore never touches.

use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// `/undo`: puts the last turn's files back, and says what it did.
    pub(super) fn command_undo(&mut self) -> Result<Option<String>, String> {
        let Some(live) = self.live.as_mut() else {
            return Err("no app is running, so nothing was restored".to_string());
        };
        for line in live.undo()? {
            self.say(&line);
        }
        // A restore is about files, and saying which half of the turn it undid keeps the other
        // half honest: the model's context still holds what was said.
        self.say(
            "the conversation is unchanged: a turn's context cannot be unwound without asking the \
             model again",
        );
        Ok(None)
    }
}
