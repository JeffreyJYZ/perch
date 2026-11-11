//! The `perch` binary: one window, or its accessibility tree when `--tree` asks for it.
//!
//! `--tree` prints the tree to stdout and exits, so "it renders" is proved by the tree. It never
//! touches settings or the keychain: the tree is the layout, with no app behind it.
//! `--offline` also skips the app, so the window opens with the session list and the dock alone.
//! `--plugins <dir>` loads plugins from a directory of its own instead of `<data>/plugins`.
//!
//! The live window is one frame at a time: [`poll`] folds what the turn and the shells have
//! posted into the shell with a small budget, and asks for the next frame only while something is
//! still moving. Nothing on the drawing thread waits for a provider, a tool or a pty.

use std::path::PathBuf;
use std::sync::Arc;

use perch_app::settings;
use perch_files::Workspace;
use perch_ui::Shell;
use rune::{AppSpec, Chrome, Env, KeyEvent, Runtime, Tx};

mod window;
use window::{Window, Zed};

/// The window's size, in points.
const WINDOW: (f32, f32) = (1280.0, 820.0);
/// The application id, shared with the keychain service name so the data directory matches it.
const APP_ID: &str = "dev.jyz.perch";

/// The flags perch reads itself. Everything else is Rune's (`--snap`, `--size`, `--theme`, …).
struct Flags {
    tree: bool,
    offline: bool,
    /// Where a `.wasm` goes to be installed. `None` is `plugins/` under the data directory.
    plugins: Option<PathBuf>,
    /// The directory a first-run session belongs to. `None` is the launch directory.
    worktree: Option<PathBuf>,
}

impl Flags {
    fn parse(args: impl IntoIterator<Item = String>) -> Flags {
        let mut flags = Flags {
            tree: false,
            offline: false,
            plugins: None,
            worktree: None,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--tree" => flags.tree = true,
                "--offline" => flags.offline = true,
                // The value follows the flag, and a missing one is no override rather than an
                // error: the default directory is always a valid answer.
                "--plugins" => flags.plugins = args.next().map(PathBuf::from),
                "--worktree" => flags.worktree = args.next().map(PathBuf::from),
                _ => {}
            }
        }
        flags
    }
}

fn main() {
    let flags = Flags::parse(std::env::args().skip(1));
    if flags.tree {
        print_tree();
        return;
    }
    // A titled window keeps AppKit's traffic lights above the shell instead of over the session
    // list: the app draws no titlebar band of its own yet.
    let chrome = Chrome {
        full_size_content: false,
        traffic_lights: None,
        drag_regions: false,
    };
    rune::App::new(AppSpec {
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("perch", WINDOW, init, perch_ui::shell::view)
    })
    .chrome(chrome)
    .run();
}

/// Opens the app, then the window over it.
fn init() -> Shell<Window> {
    let flags = Flags::parse(std::env::args().skip(1));
    let window = if flags.offline {
        Window::Offline("started with --offline".to_string())
    } else {
        open_app(&flags)
    };
    let (sessions, active) = match window.app() {
        Some(app) => (app.metas(), app.active()),
        None => (Vec::new(), 0),
    };
    let has_session = !sessions.is_empty();
    let shell = match sessions
        .get(active)
        .and_then(|session| Workspace::open(&session.worktree).ok())
    {
        Some(workspace) => Shell::with_workspace(sessions, active, workspace),
        None => Shell::new(sessions, active),
    };
    let mut shell = shell
        // The app's one editor call: the preview's button runs it, and nothing else can.
        .with_editor(Arc::new(Zed))
        .with_live(window);
    // One shell for the first tab: the dock is a real terminal from the first frame, and the pty
    // starts in the session's worktree. A window with no session has nothing to start it in, and
    // starting a shell at `/` is the same mistake a root worktree would be.
    if has_session {
        let _ = shell.open_terminal();
    }
    shell
}

/// The app behind the window: its data directory, its settings, its MCP servers, its plugins and
/// its sessions.
///
/// The plugins load last, after the app opened, and the order matters only for what a failure
/// costs: a plugin directory that will not read still leaves a window with an app and no column,
/// while an app that will not open is the window's one real failure.
fn open_app(flags: &Flags) -> Window {
    let Some(dir) = data_dir() else {
        return Window::Offline("no data directory for this platform".to_string());
    };
    let plugins = flags
        .plugins
        .clone()
        .unwrap_or_else(|| dir.join(settings::PLUGINS));
    match settings::open_with_plugins(&dir, &plugins) {
        Ok(mut app) => {
            // A first run has no session at all, and a window with nothing to ask is a dead one:
            // the launch directory is the worktree, which is the design's rule.
            if app.sessions().is_empty() {
                let worktree = match perch_app::launch::project_root(flags.worktree.clone()) {
                    Ok(path) => path,
                    Err(words) => return Window::Offline(words),
                };
                if let Err(error) = app.new_session(&worktree) {
                    return Window::Offline(error.to_string());
                }
            }
            Window::open(app)
        }
        Err(error) => Window::Offline(error.to_string()),
    }
}

/// One frame of the world: what the turn and the shells posted since the last one.
fn poll(shell: &mut Shell<Window>, _seconds: f64, tx: &mut Tx) -> bool {
    let changed = shell.pump(perch_ui::shell::BUDGET);
    if let Some(wait) = shell.next_wake() {
        tx.wake_after(wait);
    }
    changed
}

/// Keys the focused control did not take: a focused tool card's Enter, then the window's own map.
fn on_key(shell: &mut Shell<Window>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    perch_ui::shell::window_key(shell, ev, tx)
}

/// The app's data directory, or none when the platform gives no home directory.
fn data_dir() -> Option<PathBuf> {
    rune::dirs::AppDirs::new(APP_ID).map(|dirs| dirs.data().to_path_buf())
}

/// Lays the window out once and prints its accessibility tree, then exits.
///
/// The tree is the layout with nothing behind it, so this reads no settings and starts no shell.
fn print_tree() {
    let mut runtime = Runtime::new(
        Shell::idle(perch_app::launch::fallback_sessions(None), 0),
        perch_ui::shell::view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.frame(0.0);
    let tree = serde_json::to_string_pretty(&runtime.accessibility_json()).expect("a tree is JSON");
    println!("{tree}");
}
