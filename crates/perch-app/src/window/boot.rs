//! Opening the app the window draws: its data directory, its settings, its MCP servers, its
//! plugins and the session a launch starts with.

use std::path::PathBuf;

use perch_app::launch::{self, Launch};
use perch_app::settings;

use super::Window;
use crate::Flags;

/// The application id, shared with the keychain service name so the data directory matches it.
const APP_ID: &str = "dev.jyz.perch";

/// The app behind the window, and the words for a window that opens with no folder.
///
/// The plugins load last, after the app opened, and the order matters only for what a failure
/// costs: a plugin directory that will not read still leaves a window with an app and no column,
/// while an app that will not open is the window's one real failure.
///
/// A launch directory that is not a project is not a failure either: the window opens with no
/// session, and the words say why so the sidebar can offer the folder picker in its place.
pub(crate) fn open(flags: &Flags) -> (Window, Option<String>) {
    let Some(dir) = data_dir() else {
        return (
            Window::Offline("no data directory for this platform".to_string()),
            None,
        );
    };
    let plugins = flags
        .plugins
        .clone()
        .unwrap_or_else(|| dir.join(settings::PLUGINS));
    match settings::open_with_plugins(&dir, &plugins) {
        Ok(mut app) => {
            let mut notice = None;
            // An explicit worktree wins on every launch, not only the first: the flag names the
            // project this window is for, and a session for it is opened or reused. A folder
            // named this way is a person's choice, so it is never refused.
            if let Some(path) = flags.worktree.clone() {
                match launch::directory(path) {
                    Ok(path) if app.metas().iter().all(|meta| meta.worktree != path) => {
                        if let Err(error) = app.new_session(&path) {
                            notice = Some(format!("{}: {error}", path.display()));
                        }
                    }
                    Ok(_) => {}
                    Err(words) => notice = Some(words),
                }
            } else if app.sessions().is_empty() {
                // A first run has no session at all. A launch directory that is not a project —
                // `/` for a window opened from Finder — opens with no session and the picker,
                // never a window that refuses to start.
                match launch::launch(None) {
                    Launch::Project(worktree) => {
                        if let Err(error) = app.new_session(&worktree) {
                            notice = Some(format!("{}: {error}", worktree.display()));
                        }
                    }
                    Launch::Empty(words) => notice = Some(words),
                }
            }
            (Window::open(app), notice)
        }
        Err(error) => (Window::Offline(error.to_string()), None),
    }
}

/// The app's data directory, or none when the platform gives no home directory.
fn data_dir() -> Option<PathBuf> {
    rune::dirs::AppDirs::new(APP_ID).map(|dirs| dirs.data().to_path_buf())
}
