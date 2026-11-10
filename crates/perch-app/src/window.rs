//! The window's live half: the app when its data directory opened, the plugins beside it, and the
//! words when neither did.
//!
//! A window that refuses to start is worse than one that starts without a session: the layout, the
//! file panel and the dock all still work, and the reason the app is missing is on stderr. Every
//! method here forwards to the app, so the window is the same window whether or not one opened.
//!
//! The plugins sit beside the app rather than inside it because a session fixes its tool list when
//! it starts: the host owns the registry the plugin tools registered into, its panels are what the
//! right-hand column draws, and the session's events reach it through the log the app appends to.

use std::path::Path;
use std::time::Duration;

use perch_app::AppState;
use perch_app::settings;
use perch_plugins::{Loaded, PanelEntry};
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

/// What opened: the app that owns the sessions, and the plugins loaded beside it.
pub struct Open {
    app: AppState,
    plugins: Loaded,
}

/// What the window's state is, behind the layout.
pub enum Window {
    /// The app opened and owns the sessions, and the plugins beside it own the tool registry their
    /// tools registered into and the panels they declare.
    Open(Box<Open>),
    /// Nothing opened, and this is why: shown where a prompt would be.
    Offline(String),
}

impl Window {
    /// The app, with the plugins that loaded beside it.
    pub(crate) fn open(app: AppState, plugins: Loaded) -> Window {
        Window::Open(Box::new(Open { app, plugins }))
    }

    /// The app, when it opened.
    pub(crate) fn app(&self) -> Option<&AppState> {
        match self {
            Window::Open(open) => Some(&open.app),
            Window::Offline(_) => None,
        }
    }

    /// The plugin host, when the app opened: what the column draws and what the events reach.
    pub(crate) fn plugins(&self) -> Option<&Loaded> {
        match self {
            Window::Open(open) => Some(&open.plugins),
            Window::Offline(_) => None,
        }
    }

    /// Both halves, for a method that changes the app and then feeds the plugins.
    fn parts(&mut self) -> Option<(&mut AppState, &mut Loaded)> {
        match self {
            Window::Open(open) => Some((&mut open.app, &mut open.plugins)),
            Window::Offline(_) => None,
        }
    }
}

impl Live for Window {
    fn pump(&mut self, budget: Duration) -> bool {
        let Some((app, plugins)) = self.parts() else {
            return false;
        };
        let changed = Live::pump(app, budget);
        // Only while something moves: an idle window must not open the session's log every frame.
        // A turn that has just ended is the interesting case, and it makes `changed` true.
        let dispatched = (changed || app.busy()) && settings::feed(plugins, app);
        changed || dispatched
    }

    fn transcript(&self) -> Option<&Transcript> {
        self.app().and_then(Live::transcript)
    }

    fn transcript_mut(&mut self) -> Option<&mut Transcript> {
        self.parts().and_then(|(app, _)| Live::transcript_mut(app))
    }

    fn new_session(
        &mut self,
        worktree: &std::path::Path,
    ) -> std::result::Result<(usize, SessionMeta), String> {
        match self {
            Window::Open(open) => Live::new_session(&mut open.app, worktree),
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
            Window::Open(open) => Live::prompt(&mut open.app, text),
            Window::Offline(why) => Err(format!("no session is open: {why}")),
        }
    }

    fn cancel(&mut self) {
        if let Some((app, _)) = self.parts() {
            Live::cancel(app);
        }
    }

    fn approve(&mut self) {
        if let Some((app, _)) = self.parts() {
            Live::approve(app);
        }
    }

    fn deny(&mut self) {
        if let Some((app, _)) = self.parts() {
            Live::deny(app);
        }
    }

    fn pick(&mut self, index: usize) {
        if let Some((app, _)) = self.parts() {
            Live::pick(app, index);
        }
    }

    fn switch(&mut self, index: usize) {
        if let Some((app, _)) = self.parts() {
            Live::switch(app, index);
        }
    }

    fn settings(&self) -> Option<SettingsSnapshot> {
        self.app().and_then(Live::settings)
    }

    fn save_settings(&mut self, draft: SettingsDraft) -> std::result::Result<KeyStatus, String> {
        match self {
            Window::Open(open) => Live::save_settings(&mut open.app, draft),
            Window::Offline(why) => Err(format!("no app is running: {why}")),
        }
    }
}

/// The right-hand column: one panel per live plugin, then the words for what is not there.
impl Panels for Window {
    fn panels(&self) -> Vec<PanelEntry> {
        self.plugins().map(Loaded::panels).unwrap_or_default()
    }

    fn plugin_words(&self) -> Vec<String> {
        self.plugins().map(Loaded::words).unwrap_or_default()
    }
}
