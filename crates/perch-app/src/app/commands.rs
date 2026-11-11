//! The app's half of the command surface: names, removal and the saved theme.
//!
//! Each is a real effect on the state and the log, never a hint: a rename rewrites the session's
//! header, a delete closes the log and drops the runtime, a theme save writes `settings.toml`.
//! The auto-title job lives in [`title`](super::title).

use perch_store::SessionMeta;

use super::{AppState, Error, Result};
use crate::settings as file;

impl AppState {
    /// Names the session at `index`, writes the new header, and answers the header it now holds.
    pub fn rename_session(&mut self, index: usize, title: &str) -> Result<SessionMeta> {
        let Some(runtime) = self.sessions.get_mut(index) else {
            return Err(Error::MissingSession(index));
        };
        runtime.meta.title = title.trim().to_string();
        runtime.log.set_title(title)?;
        Ok(runtime.meta.clone())
    }

    /// Removes the session at `index`: its bridge stops, its runtime drops, its log is deleted.
    /// Answers the index the window should show afterwards.
    pub fn delete_session(&mut self, index: usize) -> Result<usize> {
        if index >= self.sessions.len() {
            return Err(Error::MissingSession(index));
        }
        let runtime = self.sessions.remove(index);
        // The turn stops first, so nothing is still writing to a log that is about to go. The
        // channels close when the runtime drops, which is what ends the dispatch thread.
        runtime.agent.cancel();
        let id = runtime.meta.id.clone();
        let log = runtime.log;
        log.remove()?;
        self.titles_forget(&id);
        if self.active > index {
            self.active -= 1;
        }
        self.active = self.active.min(self.sessions.len().saturating_sub(1));
        Ok(self.active)
    }

    /// Saves `theme` to `settings.toml`, keeping every other field as it was.
    pub fn save_theme(&mut self, theme: &str) -> Result<()> {
        if !perch_ui::THEMES.contains(&theme) {
            return Err(Error::Model(format!(
                "theme must be one of {}",
                perch_ui::THEMES.join(", ")
            )));
        }
        let snapshot = file::snapshot(&self.dir, &*self.secrets);
        let draft = perch_ui::SettingsDraft {
            base_url: snapshot.base_url,
            model: snapshot.model,
            theme: theme.to_string(),
            update_check: snapshot.update_check,
            api_key: String::new(),
        };
        file::apply(&self.dir, &mut self.defaults, &*self.secrets, draft)?;
        Ok(())
    }
}
