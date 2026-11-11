//! The frame handoff: the terminal thread tells the window a frame is ready.
//!
//! The frame channel cannot say "there is one", so a window that only reads it on a timer draws a
//! shell's output one tick late — measured at up to 16.7 ms, the `SHELL_TICK` floor. `spawn_with_wake`
//! is the answer; this proves the callback runs once a frame has been published, and that `spawn`
//! (the no-wake form every other caller uses) still starts a shell.

use std::{
    fs,
    io::Write as _,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use perch_term::TermSession;

/// `$SHELL` is process-wide, and this binary spawns through it: one at a time.
static SHELL: Mutex<()> = Mutex::new(());

#[test]
fn a_published_frame_wakes_the_window() {
    // The first frame is published as soon as the emulator exists, so this needs no keystroke and
    // no particular shell behaviour: the wake is the same call an echoed character would make.
    let (_dir, script) = fixture("wake", "exec cat\n");
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&wakes);

    let _held = SHELL.lock().expect("the shell lock");
    let previous = std::env::var_os("SHELL");
    // SAFETY: the lock is held for the whole spawn, so nothing else in this binary reads
    // `$SHELL` while it names the fixture.
    unsafe { std::env::set_var("SHELL", &script) };
    let spawned = TermSession::spawn_with_wake(Path::new("/"), 40, 8, move || {
        counted.fetch_add(1, Ordering::Release);
    });
    match previous {
        Some(value) => unsafe { std::env::set_var("SHELL", value) },
        None => unsafe { std::env::remove_var("SHELL") },
    }
    // The handle stays alive for the whole test: dropping it shuts the shell down.
    let (_handle, _frames) = spawned.expect("spawn the fixture shell");

    let deadline = Instant::now() + Duration::from_secs(5);
    while wakes.load(Ordering::Acquire) == 0 {
        assert!(
            Instant::now() < deadline,
            "the terminal thread published a frame and never woke the window"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    eprintln!(
        "the first published frame woke the window {} time(s)",
        wakes.load(Ordering::Acquire)
    );
}

/// Write an executable `sh` script and keep its directory alive.
fn fixture(name: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("create a scratch directory");
    let path = dir.path().join(name);
    let mut file = fs::File::create(&path).expect("create the fixture script");
    file.write_all(format!("#!/bin/sh\n{body}").as_bytes())
        .expect("write the fixture script");
    drop(file);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("make the fixture script executable");
    (dir, path)
}
