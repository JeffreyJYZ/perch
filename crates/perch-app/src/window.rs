//! The window's live half: the app when its data directory opened, and the words when it did not.
//!
//! A window that refuses to start is worse than one that starts without a session: the layout, the
//! file panel and the dock all still work, and the reason the app is missing is on stderr. Every
//! method here forwards to the app, so the window is the same window whether or not one opened.

use std::time::Duration;

use perch_app::AppState;
use perch_ui::SettingsDraft;
use perch_ui::Transcript;
use perch_ui::shell::{KeyStatus, Live, SettingsSnapshot, Waiting};

/// What the window's state is, behind the layout.
pub enum Window {
    /// The app opened and owns the sessions.
    Open(AppState),
    /// Nothing opened, and this is why: shown where a prompt would be.
    Offline(String),
}

impl Window {
    /// The app, when it opened.
    pub(crate) fn app(&self) -> Option<&AppState> {
        match self {
            Window::Open(app) => Some(app),
            Window::Offline(_) => None,
        }
    }
}

impl Live for Window {
    fn pump(&mut self, budget: Duration) -> bool {
        match self {
            Window::Open(app) => Live::pump(app, budget),
            Window::Offline(_) => false,
        }
    }

    fn transcript(&self) -> Option<&Transcript> {
        self.app().and_then(Live::transcript)
    }

    fn waiting(&self) -> Waiting {
        self.app().map_or(Waiting::Nothing, Live::waiting)
    }

    fn busy(&self) -> bool {
        self.app().is_some_and(Live::busy)
    }

    fn prompt(&mut self, text: String) -> std::result::Result<(), String> {
        match self {
            Window::Open(app) => Live::prompt(app, text),
            Window::Offline(why) => Err(format!("no session is open: {why}")),
        }
    }

    fn cancel(&mut self) {
        if let Window::Open(app) = self {
            Live::cancel(app);
        }
    }

    fn approve(&mut self) {
        if let Window::Open(app) = self {
            Live::approve(app);
        }
    }

    fn deny(&mut self) {
        if let Window::Open(app) = self {
            Live::deny(app);
        }
    }

    fn pick(&mut self, index: usize) {
        if let Window::Open(app) = self {
            Live::pick(app, index);
        }
    }

    fn switch(&mut self, index: usize) {
        if let Window::Open(app) = self {
            Live::switch(app, index);
        }
    }

    fn settings(&self) -> Option<SettingsSnapshot> {
        self.app().and_then(Live::settings)
    }

    fn save_settings(&mut self, draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        match self {
            Window::Open(app) => Live::save_settings(app, draft),
            Window::Offline(why) => Err(format!("no app is running: {why}")),
        }
    }
}
