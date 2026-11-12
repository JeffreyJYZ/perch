//! The typing path, measured: a key press reaches the shell, and how long its echo takes.
//!
//! Two shells are measured, because the two halves of the cost have different owners. `cat`
//! under `/bin/sh` echoes a byte the moment the kernel's tty sees it, so what it measures is
//! perch's own path: channel, write, pty, reader thread, emulator, frame. `zsh -f -i` adds the
//! shell's own line editor, which is the part perch cannot make faster — when the two are close,
//! the shell is not the reason typing feels late.

use std::{
    fs,
    io::Write as _,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use perch_term::{Frame, Key, KeyEvent, TermHandle, TermSession};
/// `$SHELL` is process-wide and these tests spawn shells through it: one at a time.
static SHELL: Mutex<()> = Mutex::new(());

/// The grid the shells are spawned at.
const COLS: u16 = 40;
const ROWS: u16 = 8;
/// How many keystrokes one measurement types.
const KEYS: usize = 20;

#[test]
fn a_key_press_reaches_the_shell() {
    // `read` returns only when a line arrives, so a Return that is encoded but never written
    // leaves the shell waiting forever and no frame ever says what it read.
    let (_dir, script) = fixture("read-a-line", "read line\nprintf 'got:%s\\n' \"$line\"\n");
    let (handle, frames) = spawn(&script);

    handle.write(b"abc");
    handle.send_key(KeyEvent::plain(Key::Enter));

    let said = next_frame_where(
        &frames,
        |frame| text(frame).contains("got:abc"),
        Duration::from_secs(5),
    );
    assert!(
        text(&said).contains("got:abc"),
        "the shell read the line the keys sent: {:?}",
        text(&said)
    );
}

#[test]
fn a_keystroke_echoes_within_the_budget_the_dock_documents() {
    for (label, body) in [
        ("sh tty echo", "exec cat\n"),
        ("zsh line editor", "exec /bin/zsh -f -i\n"),
    ] {
        let timings = measure(label, body);
        // The cost perch owns: the shell writes an echo once it reads the byte, and the terminal
        // thread must have it in a frame. The median is asserted and the worst is printed, because
        // this runs beside other tests: one descheduled read says nothing, a median does.
        let median = timings[timings.len() / 2];
        assert!(
            median < Duration::from_millis(120),
            "{label}: median echo {median:?} is past a frame's own budget"
        );
    }
}

/// Type [`KEYS`] characters at `body`'s shell and print how long each echo took to arrive.
fn measure(label: &str, body: &str) -> Vec<Duration> {
    let (_dir, script) = fixture("typing", body);
    let (handle, frames) = spawn(&script);
    // The prompt (zsh has one) is what the caret sits after; every echo moves it one cell on.
    let first = next_frame_where(&frames, |_| true, Duration::from_secs(10));
    let baseline = i64::from(first.cursor.col);

    let mut timings = Vec::with_capacity(KEYS);
    for index in 0..KEYS {
        drain(&frames);
        let column = baseline + i64::try_from(index + 1).expect("a small count");
        let started = Instant::now();
        handle.write(b"a");
        let _ = next_frame_where(
            &frames,
            |frame| i64::from(frame.cursor.col) >= column,
            Duration::from_secs(5),
        );
        timings.push(started.elapsed());
    }
    timings.sort_unstable();
    let at = |fraction: f64| {
        let index = ((timings.len() - 1) as f64 * fraction).round() as usize;
        timings[index]
    };
    eprintln!(
        "{label}: write -> frame over {KEYS} keys: p50 {:?}, p90 {:?}, max {:?}",
        at(0.5),
        at(0.9),
        at(1.0)
    );
    timings
}

/// Every glyph a frame holds, in row order.
fn text(frame: &Frame) -> String {
    frame
        .rows
        .iter()
        .map(|row| row.cells.iter().map(|cell| cell.text.as_str()).collect())
        .collect::<Vec<String>>()
        .join("\n")
}

/// Drop the frames already queued, so a measurement times the write it just made.
fn drain(frames: &Receiver<Frame>) {
    while frames.try_recv().is_ok() {}
}

/// Spawn `script` as the shell at [`COLS`] x [`ROWS`], restoring `$SHELL` afterwards.
fn spawn(script: &Path) -> (TermHandle, Receiver<Frame>) {
    let _held = SHELL.lock().expect("the shell lock");
    let previous = std::env::var_os("SHELL");
    // SAFETY: the lock is held for the whole spawn, so nothing else in this binary reads
    // `$SHELL` while it names the fixture.
    unsafe { std::env::set_var("SHELL", script) };
    let spawned = TermSession::spawn(Path::new("/"), COLS, ROWS);
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

/// Read frames until `accept` says yes, or fail after `within`.
fn next_frame_where(
    frames: &Receiver<Frame>,
    accept: impl Fn(&Frame) -> bool,
    within: Duration,
) -> Frame {
    let deadline = Instant::now() + within;
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
