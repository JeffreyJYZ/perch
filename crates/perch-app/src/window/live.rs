//! The window's forwarding half: every [`Live`] method answered by the app behind it, or by the
//! words saying why there is none.
//!
//! Split from [`window`](super) so the type and the editor stay one short file. The methods whose
//! only job is "the app's answer, or the offline sentence" are generated from one list, so the
//! same reasoning is not retyped for each of them.

use std::time::Duration;

use perch_agent::ListedModel;
use perch_store::SessionMeta;
use perch_ui::SettingsDraft;
use perch_ui::Transcript;
use perch_ui::shell::{KeyStatus, Live, SettingsSnapshot, Waiting};

use super::Window;

/// One forwarder per method that is the app's answer or the offline sentence.
///
/// Every entry names its arguments and its success type; the match on the window is the same
/// shape for all of them, so it is written once here rather than nine times below.
macro_rules! forwarded {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(
            fn $name(&mut self $(, $arg: $ty)*) -> std::result::Result<$ret, String> {
                let why = self.why_not();
                match self.app_mut() {
                    Some(app) => Live::$name(app $(, $arg)*),
                    None => Err(why),
                }
            }
        )*
    };
}

impl Live for Window {
    fn pump(&mut self, budget: Duration) -> bool {
        match self {
            Window::Open(app) => Live::pump(app.as_mut(), budget),
            Window::Offline(_) => false,
        }
    }

    fn transcript(&self) -> Option<&Transcript> {
        self.app().and_then(Live::transcript)
    }

    fn transcript_mut(&mut self) -> Option<&mut Transcript> {
        self.app_mut().and_then(Live::transcript_mut)
    }

    fn waiting(&self) -> Waiting {
        self.app().map_or(Waiting::Nothing, Live::waiting)
    }

    fn busy(&self) -> bool {
        self.app().is_some_and(Live::busy)
    }

    forwarded! {
        new_session(worktree: &std::path::Path) -> (usize, SessionMeta);
        open_folder(path: &std::path::Path) -> (usize, SessionMeta);
        prompt(text: String) -> ();
        models() -> Vec<ListedModel>;
        choose_model(id: String) -> ();
        rename_session(index: usize, title: &str) -> SessionMeta;
        delete_session(index: usize) -> usize;
        set_theme(theme: &str) -> ();
        save_settings(draft: SettingsDraft) -> KeyStatus;
    }

    fn cancel(&mut self) {
        if let Some(app) = self.app_mut() {
            Live::cancel(app);
        }
    }

    fn approve(&mut self) {
        if let Some(app) = self.app_mut() {
            Live::approve(app);
        }
    }

    fn allow_for_session(&mut self) {
        if let Some(app) = self.app_mut() {
            Live::allow_for_session(app);
        }
    }

    fn deny(&mut self) {
        if let Some(app) = self.app_mut() {
            Live::deny(app);
        }
    }

    fn pick(&mut self, index: usize) {
        if let Some(app) = self.app_mut() {
            Live::pick(app, index);
        }
    }

    fn pick_many(&mut self, picks: &[usize]) {
        if let Some(app) = self.app_mut() {
            Live::pick_many(app, picks);
        }
    }

    fn answer_text(&mut self, text: String) {
        if let Some(app) = self.app_mut() {
            Live::answer_text(app, text);
        }
    }

    fn switch(&mut self, index: usize) {
        if let Some(app) = self.app_mut() {
            Live::switch(app, index);
        }
    }

    fn session(&self, index: usize) -> Option<SessionMeta> {
        self.app().and_then(|app| Live::session(app, index))
    }

    fn model(&self) -> Option<String> {
        self.app().and_then(Live::model)
    }

    fn start_models(&mut self) {
        if let Some(app) = self.app_mut() {
            Live::start_models(app);
        }
    }

    fn models_ready(&mut self) -> Option<std::result::Result<Vec<ListedModel>, String>> {
        self.app_mut().and_then(Live::models_ready)
    }

    fn model_limit(&self) -> Option<u32> {
        self.app().and_then(Live::model_limit)
    }

    fn tokens_used(&self) -> Option<u32> {
        self.app().and_then(Live::tokens_used)
    }

    fn turn_cost(&self) -> Option<f64> {
        self.app().and_then(Live::turn_cost)
    }

    fn compact(&mut self) {
        if let Some(app) = self.app_mut() {
            Live::compact(app);
        }
    }

    fn data_dir(&self) -> Option<std::path::PathBuf> {
        self.app().and_then(Live::data_dir)
    }

    fn settings(&self) -> Option<SettingsSnapshot> {
        self.app().and_then(Live::settings)
    }

    /// The app as the shell asks for it: the window has none when it has no data directory, and
    /// then the built-in dark appearance is what it draws with.
    fn appearance(&self) -> Option<perch_ui::theme::Appearance> {
        self.app().and_then(Live::appearance)
    }
}
