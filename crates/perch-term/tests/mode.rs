//! Asking the terminal thread what a program turned on.
//!
//! A mode lives in the emulator, which only the terminal thread may touch, so
//! the question has to cross the channel and come back as an answer. These
//! tests drive that round trip with a real shell: one that prints the escape
//! sequences a full-screen program would print, and one that exits before it is
//! asked anything.

use std::{
    fs,
    io::Write as _,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use perch_term::{Frame, Mode, TermHandle, TermSession};

/// `SHELL` is process-wide and `spawn` reads it, so tests that point it at a
/// fixture must not overlap.
static SHELL: Mutex<()> = Mutex::new(());

#[test]
fn bracketed_paste_follows_what_the_program_asks_for() {
    // A program that enables the mode, holds it, then gives it back — the same
    // pair of sequences a readline front end sends around a prompt.
    let (_dir, script) = fixture(
        "mode-flip",
        "printf '\\033[?2004h'\nsleep 0.4\nprintf '\\033[?2004l'\nsleep 10\n",
    );
    let (handle, _frames) = spawn(&script);

    wait_for(&handle, true, "the program enabled bracketed paste");
    wait_for(&handle, false, "the program gave bracketed paste back");
    assert!(
        !handle.mode(Mode::BRACKETED_PASTE),
        "the mode stays off once the program turned it off"
    );
    // A mode the program never touched is off too, which is what makes `false`
    // the safe answer rather than a guess. DECCKM is the one a full-screen
    // program turns on for its own key handling, so it is the realistic pair to
    // the paste mode above.
    assert!(!handle.mode(Mode::DECCKM));
}

#[test]
fn an_exited_shell_answers_false_instead_of_hanging() {
    let (_dir, script) = fixture("mode-exit", "exit 0\n");
    let (mut handle, _frames) = spawn(&script);

    let deadline = Instant::now() + Duration::from_secs(10);
    while !handle.exited() {
        assert!(Instant::now() < deadline, "the fixture shell never exited");
        std::thread::sleep(Duration::from_millis(10));
    }

    let started = Instant::now();
    let bracketed = handle.mode(Mode::BRACKETED_PASTE);
    let waited = started.elapsed();
    eprintln!("mode after the shell exited: {bracketed} in {waited:?}");

    assert!(!bracketed, "a shell that exited never enabled the mode");
    assert!(
        waited < Duration::from_secs(1),
        "asking a dead shell took {waited:?}, which is a hang, not an answer"
    );
}

/// Spawn a shell in a scratch directory, with `SHELL` pointing at `script`.
fn spawn(script: &Path) -> (TermHandle, Receiver<Frame>) {
    let _guard = SHELL.lock().unwrap_or_else(|poison| poison.into_inner());
    let previous = std::env::var_os("SHELL");
    // SAFETY: the guard above serialises every test in this file that touches
    // `SHELL`, and `spawn` is the only reader; the value is restored before the
    // guard is released.
    unsafe { std::env::set_var("SHELL", script) };
    let spawned = TermSession::spawn(Path::new("/"), 40, 12);
    match previous {
        Some(value) => unsafe { std::env::set_var("SHELL", value) },
        None => unsafe { std::env::remove_var("SHELL") },
    }
    spawned.expect("spawn the fixture shell")
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

/// Poll until the mode reads `wanted`, or fail with what it said instead.
fn wait_for(handle: &TermHandle, wanted: bool, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let seen = handle.mode(Mode::BRACKETED_PASTE);
        if seen == wanted {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "waited for {what}, but bracketed paste still reads {seen}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
