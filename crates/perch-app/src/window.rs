//! The window's live half: the app when its data directory opened, and the words when it did not.
//!
//! A window that refuses to start is worse than one that starts without a session: the layout, the
//! file panel and the dock all still work, and the reason the app is missing is on stderr. Every
//! method here forwards to the app, so the window is the same window whether or not one opened.
//!
//! The plugins live inside the app now: its sources own the host and the panels, and its live
//! registry is the one every session dispatches through, so a window laid out over `AppState`
//! alone shows the same column.

use std::path::Path;
use std::time::Duration;

use perch_app::AppState;
use perch_plugins::PanelEntry;
use perch_store::SessionMeta;
use perch_ui::SettingsDraft;
use perch_ui::Transcript;
use perch_ui::files::Editor;
use perch_ui::shell::{KeyStatus, Live, Panels, SettingsSnapshot, Waiting};

/// The app's editor: Zed, through `perch_files`'s one `open -a` call.
///
/// This is the only place in the window's wiring that can start a process, and it is a command
/// with a path argument — never a shell string.
pub struct Zed;

impl Editor for Zed {
    fn open_in_zed(&self, path: &Path) -> std::result::Result<(), String> {
        perch_files::open_in_zed(path).map_err(|error| error.to_string())
    }
}

/// What the window's state is, behind the layout.
pub enum Window {
    /// The app opened and owns the sessions, the tools and the plugin column.
    Open(Box<AppState>),
    /// Nothing opened, and this is why: shown where a prompt would be.
    Offline(String),
}

impl Window {
    /// The app, when it opened.
    pub(crate) fn open(app: AppState) -> Window {
        Window::Open(Box::new(app))
    }

    /// The app, when it opened.
    pub(crate) fn app(&self) -> Option<&AppState> {
        match self {
            Window::Open(app) => Some(app),
            Window::Offline(_) => None,
        }
    }

    fn app_mut(&mut self) -> Option<&mut AppState> {
        match self {
            Window::Open(app) => Some(app),
            Window::Offline(_) => None,
        }
    }
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

    fn new_session(
        &mut self,
        worktree: &std::path::Path,
    ) -> std::result::Result<(usize, SessionMeta), String> {
        match self {
            Window::Open(app) => Live::new_session(app.as_mut(), worktree),
            Window::Offline(why) => Err(format!("no app is running: {why}")),
        }
    }

    fn waiting(&self) -> Waiting {
        self.app().map_or(Waiting::Nothing, Live::waiting)
    }

    fn busy(&self) -> bool {
        self.app().is_some_and(Live::busy)
    }

    fn prompt(&mut self, text: String) -> std::result::Result<(), String> {
        match self {
            Window::Open(app) => Live::prompt(app.as_mut(), text),
            Window::Offline(why) => Err(format!("no session is open: {why}")),
        }
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

    fn settings(&self) -> Option<SettingsSnapshot> {
        self.app().and_then(Live::settings)
    }

    fn save_settings(&mut self, draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        match self {
            Window::Open(app) => Live::save_settings(app.as_mut(), draft),
            Window::Offline(why) => Err(format!("no app is running: {why}")),
        }
    }
}

/// The right-hand column: one panel per live plugin, then the words for what is not there.
impl Panels for Window {
    fn panels(&self) -> Vec<PanelEntry> {
        self.app().map(Panels::panels).unwrap_or_default()
    }

    fn plugin_words(&self) -> Vec<String> {
        self.app().map(Panels::plugin_words).unwrap_or_default()
    }

    fn panel_action(
        &mut self,
        plugin: &str,
        control: &str,
        value: Option<String>,
    ) -> std::result::Result<(), String> {
        match self {
            Window::Open(app) => Panels::panel_action(app.as_mut(), plugin, control, value),
            Window::Offline(why) => Err(format!(
                "no app is running, so `{control}` reached no plugin: {why}"
            )),
        }
    }
}
