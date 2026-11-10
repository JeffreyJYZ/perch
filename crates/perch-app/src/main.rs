//! The `perch` binary: one window, or its accessibility tree when `--tree` asks for it.
//!
//! `--tree` prints the tree to stdout and exits, so "it renders" is proved by the tree. It never
//! touches settings or the keychain: the tree is the layout, with no app behind it.
//! `--offline` also skips the app, so the window opens with the session list and the dock alone.
//!
//! The live window is one frame at a time: [`poll`] folds what the turn and the shells have
//! posted into the shell with a small budget, and asks for the next frame only while something is
//! still moving. Nothing on the drawing thread waits for a provider, a tool or a pty.

use std::path::PathBuf;
use std::sync::Arc;

use perch_app::AppState;
use perch_files::Workspace;
use perch_store::SessionMeta;
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
    let offline = std::env::args().any(|arg| arg == "--offline");
    let window = if offline {
        Window::Offline("started with --offline".to_string())
    } else {
        open_app()
    };
    let (sessions, active) = match window.app() {
        Some(app) => (app.metas(), app.active()),
        None => (fallback_sessions(), 0),
    };
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
    // starts in the session's worktree.
    let _ = shell.open_terminal();
    shell
}

/// The app behind the window: its data directory, its settings, its MCP servers and its sessions.
fn open_app() -> Window {
    let Some(dir) = data_dir() else {
        return Window::Offline("no data directory for this platform".to_string());
    };
    match AppState::open(&dir) {
        Ok(mut app) => {
            // A first run has no session at all, and a window with nothing to ask is a dead one:
            // the launch directory is the worktree, which is the design's rule.
            if app.sessions().is_empty() {
                let worktree = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                if let Err(error) = app.new_session(&worktree) {
                    return Window::Offline(error.to_string());
                }
            }
            Window::Open(app)
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

/// The session list a window with no app shows: the worktree the app was launched in.
///
/// The design's rule is that a session belongs to a worktree that already exists, so the launch
/// directory is the one thing that is always true without reading anything.
fn fallback_sessions() -> Vec<SessionMeta> {
    let worktree = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let title = worktree.file_name().map_or_else(
        || "perch".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    vec![SessionMeta {
        id: "current".to_string(),
        title,
        worktree,
        branch: String::new(),
        model: "no model".to_string(),
        created: 0,
    }]
}

/// Lays the window out once and prints its accessibility tree, then exits.
///
/// The tree is the layout with nothing behind it, so this reads no settings and starts no shell.
fn print_tree() {
    let mut runtime = Runtime::new(
        Shell::idle(fallback_sessions(), 0),
        perch_ui::shell::view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.frame(0.0);
    let tree = serde_json::to_string_pretty(&runtime.accessibility_json()).expect("a tree is JSON");
    println!("{tree}");
}
