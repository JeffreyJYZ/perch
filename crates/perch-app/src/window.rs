//! The window's live half: the app when its data directory opened, and the words when it did not.
//!
//! A window that refuses to start is worse than one that starts without a session: the layout, the
//! file panel and the dock all still work, and the reason the app is missing is on stderr. Every
//! method here forwards to the app, so the window is the same window whether or not one opened.
//! The plugins live inside the app now, so a window laid out over `AppState` alone shows the same
//! column.

use std::path::Path;

use perch_app::AppState;
use perch_ui::files::Editor;

mod boot;
mod live;
pub(crate) use boot::open;

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

    /// The sentence a window with no app answers with. Empty when one is open, where a caller
    /// reads it before the borrow and only the missing case hands it back.
    fn why_not(&self) -> String {
        match self {
            Window::Open(_) => String::new(),
            Window::Offline(why) => format!("no app is running: {why}"),
        }
    }
}
