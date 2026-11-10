//! The `perch` binary: one window, or its accessibility tree when `--tree` asks for it.
//!
//! `--tree` prints the tree to stdout and exits, so "it renders" is proved by the tree. It never
//! touches settings or the keychain, which is what `--offline` would have skipped anyway.
//! `--offline` also skips the settings read the window would otherwise make, so no provider
//! setup runs at all.

use std::path::PathBuf;
use std::sync::OnceLock;

use perch_files::Workspace;
use perch_store::{SessionMeta, Settings};
use perch_ui::Shell;
use rune::{AppSpec, Chrome, Env, Runtime};

/// The window's size, in points.
const WINDOW: (f32, f32) = (1280.0, 820.0);
/// The application id, shared with the keychain service name so the data directory matches it.
const APP_ID: &str = "dev.jyz.perch";

/// The sessions the window opens with. `AppSpec::init` takes a plain fn, so the list travels
/// through a static rather than a closure.
static SESSIONS: OnceLock<Vec<SessionMeta>> = OnceLock::new();

/// The flags perch reads itself. Everything else is Rune's (`--snap`, `--size`, `--theme`, …).
struct Flags {
    tree: bool,
    offline: bool,
}

impl Flags {
    fn parse(args: impl IntoIterator<Item = String>) -> Flags {
        let mut flags = Flags {
            tree: false,
            offline: false,
        };
        for arg in args {
            match arg.as_str() {
                "--tree" => flags.tree = true,
                "--offline" => flags.offline = true,
                _ => {}
            }
        }
        flags
    }
}

fn main() {
    let flags = Flags::parse(std::env::args().skip(1));
    // A headless dump reads nothing it does not have to: no settings, no keychain, no provider.
    let settings = if flags.offline || flags.tree {
        None
    } else {
        load_settings()
    };
    let sessions = start_sessions(settings.as_ref());
    if flags.tree {
        print_tree(&sessions);
        return;
    }
    let _ = SESSIONS.set(sessions);
    // A titled window keeps AppKit's traffic lights above the shell instead of over the session
    // list: the app draws no titlebar band of its own yet.
    let chrome = Chrome {
        full_size_content: false,
        traffic_lights: None,
        drag_regions: false,
    };
    rune::App::new(AppSpec::new("perch", WINDOW, init, perch_ui::shell::view))
        .chrome(chrome)
        .run();
}

fn init() -> Shell {
    open_window(SESSIONS.get().cloned().unwrap_or_default())
}

/// The window over `sessions`: the active session's worktree is read once, so the file panel
/// shows the tree the session belongs to. A worktree that cannot be opened leaves the column
/// saying so rather than stopping the window from starting.
fn open_window(sessions: Vec<SessionMeta>) -> Shell {
    match sessions
        .first()
        .and_then(|session| Workspace::open(&session.worktree).ok())
    {
        Some(workspace) => Shell::with_workspace(sessions, 0, workspace),
        None => Shell::new(sessions, 0),
    }
}

/// The app's data directory, or none when the platform gives no home directory.
fn data_dir() -> Option<PathBuf> {
    rune::dirs::AppDirs::new(APP_ID).map(|dirs| dirs.data().to_path_buf())
}

/// The settings the window starts from. A failure is not fatal: the window opens with defaults
/// rather than refusing to start.
fn load_settings() -> Option<Settings> {
    Settings::load(&data_dir()?).ok()
}

/// The sessions the window opens with.
///
/// Until the session logs are read at start, this is one session bound to the worktree the app
/// was launched in — the design's rule that a session belongs to a worktree that already exists.
/// `settings` is `None` offline, which the row shows as the model it has: none.
fn start_sessions(settings: Option<&Settings>) -> Vec<SessionMeta> {
    let worktree = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let title = worktree.file_name().map_or_else(
        || "perch".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let model = settings.map_or_else(|| "no model".to_string(), |settings| settings.model.clone());
    vec![SessionMeta {
        id: "current".to_string(),
        title,
        worktree,
        branch: String::new(),
        model,
        created: 0,
    }]
}

/// Lays the window out once and prints its accessibility tree, then exits.
fn print_tree(sessions: &[SessionMeta]) {
    let mut runtime = Runtime::new(
        open_window(sessions.to_vec()),
        perch_ui::shell::view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.frame(0.0);
    let tree = serde_json::to_string_pretty(&runtime.accessibility_json()).expect("a tree is JSON");
    println!("{tree}");
}
