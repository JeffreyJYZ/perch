//! The plugin fixtures and the app the `perch-app` tests open over them.
//!
//! Not a test binary itself: only `.rs` files directly under `tests/` are, and each of those takes
//! the part of this it needs.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use perch_app::{AppState, ProviderDefaults};
use perch_store::secret::MemorySecrets;
use perch_store::{SessionLog, SessionMeta};
use tempfile::TempDir;

/// The checked-in plugin fixtures live beside the sibling crate.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../perch-plugins/fixtures")
}

/// A throwaway plugin directory holding copies of `files`, named `(from, to)`.
pub fn dir_with(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (from, to) in files {
        std::fs::copy(fixtures().join(from), dir.path().join(to)).unwrap();
    }
    dir
}

/// A session header in `dir`, so the app opens a session over it.
pub fn session(dir: &Path) -> SessionLog {
    SessionLog::open(
        dir,
        SessionMeta {
            id: "s1".to_string(),
            title: "s".to_string(),
            worktree: dir.to_path_buf(),
            branch: String::new(),
            model: "test/model".to_string(),
            created: 0,
        },
    )
    .expect("the session log opens")
}

/// Tests must not read the machine they run on.
///
/// A real `~/.config/opencode/AGENTS.md` joins every session's system prompt, so the machine's own
/// notes would decide what the column says and a test would pass on one computer and fail on the
/// next. `PERCH_HOME` is the seam `perch_agent::instructions` documents for exactly this.
pub fn hermetic() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = std::env::temp_dir().join("perch-tests-home");
        let _ = std::fs::create_dir_all(&home);
        // Safety: set once per process, before any app is built, and only under test.
        unsafe { std::env::set_var("PERCH_HOME", &home) };
    });
}

/// An app over `dir` that never reaches the keychain and never makes a request.
pub fn app(dir: &Path) -> AppState {
    hermetic();
    AppState::open_with_secrets(
        dir,
        ProviderDefaults {
            base_url: "http://127.0.0.1:1/v1".to_string(),
            api_key: String::new(),
            model: "test/model".to_string(),
        },
        Box::new(MemorySecrets::default()),
    )
    .expect("the window opens over the data directory")
}
