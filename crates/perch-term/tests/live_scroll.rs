//! The live path: scrolling the viewport the terminal thread owns.
//!
//! A fixture shell prints a known 200 lines, so the frame the UI receives proves
//! the request was applied rather than only acknowledged.

mod support;

use std::{
    fs,
    io::Write as _,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use perch_term::{Frame, TermHandle, TermSession};
use support::line;

/// The offset the live scroll test moves by.
const UP: usize = 10;

#[test]
fn the_live_handle_scrolls_to_history_and_back() {
    // 200 numbered lines, then the shell exits: the history stays readable
    // after it is gone, because the terminal thread outlives the process.
    let (_dir, script) = fixture(
        "scroll-lines",
        "i=0\nwhile [ $i -lt 200 ]; do echo \"line $i\"; i=$((i+1)); done\nexit 0\n",
    );
    let (mut handle, frames) = spawn(&script);

    // Wait for the last page of output, so the live bottom is the known
    // 196..200 window before asking for history.
    let live = next_frame_where(&frames, |frame| line(frame, 0) == "line 196");
    assert_eq!(line(&live, 4), "");

    let started = Instant::now();
    assert!(
        handle.scroll(UP),
        "the terminal thread answers a scroll request"
    );
    let scrolled = next_frame_where(&frames, |frame| line(frame, 0) == "line 186");
    assert_eq!(
        line(&scrolled, 4),
        "line 190",
        "ten rows up is the same window, ten rows shifted"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a scroll request is bounded by its timeout, not by the flood"
    );

    assert!(
        handle.scroll(0),
        "returning to the live bottom is answered too"
    );
    let _back = next_frame_where(&frames, |frame| line(frame, 0) == "line 196");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !handle.exited() {
        assert!(Instant::now() < deadline, "the fixture shell never exited");
        std::thread::sleep(Duration::from_millis(10));
    }

    // Only a thread that is gone answers `false`; an exited shell still has a
    // useful history, and the request stays bounded either way.
    let started = Instant::now();
    let answered = handle.scroll(UP);
    let waited = started.elapsed();
    eprintln!("scroll after the shell exited: {answered} in {waited:?}");
    assert!(
        answered,
        "the thread still owns the history after the shell exits"
    );
    assert!(
        waited < Duration::from_secs(1),
        "asking a dead shell took {waited:?}, which is a hang, not an answer"
    );
}

/// Spawn the shell at `script` at 20x5, restoring `SHELL` afterwards.
fn spawn(script: &Path) -> (TermHandle, Receiver<Frame>) {
    let previous = std::env::var_os("SHELL");
    // SAFETY: this file has a single test, so nothing else reads `SHELL` while
    // it points at the fixture, and the value is restored before `spawn` returns.
    unsafe { std::env::set_var("SHELL", script) };
    let spawned = TermSession::spawn(Path::new("/"), 20, 5);
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

/// Read frames until `accept` says yes, or fail after a bounded wait.
fn next_frame_where(frames: &Receiver<Frame>, accept: fn(&Frame) -> bool) -> Frame {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let frame = frames
            .recv_timeout(left)
            .expect("a frame from the terminal thread before the deadline");
        if accept(&frame) {
            return frame;
        }
    }
}
