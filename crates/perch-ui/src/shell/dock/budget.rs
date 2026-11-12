//! What one echoed keystroke costs the window after its frame arrives.
//!
//! The terminal thread's half of the round trip is measured in `perch-term` (`tests/typing.rs`,
//! about 0.06 ms). What is left is this side: install the frame, re-run the view, lay it out and
//! build it. That cost, once per echo, is what decides whether forty characters a second can be
//! drawn on the frame an echo lands on or whether they queue behind each other.

use std::time::{Duration, Instant};

use perch_term::replay;
use rune::{Env, Runtime};

use super::super::Shell;
use crate::shell::view;

/// The window the dock lays its grid out in, and the grid an echo is measured at.
const WINDOW: (f32, f32) = (1280.0, 820.0);
const COLS: u16 = 100;
const ROWS: u16 = 12;
/// How many keystrokes one measurement commits.
const KEYS: usize = 20;

/// A commit's budget in the build that ships, and in a cargo dev profile.
///
/// The dev profile is not the acceptance number: the vendored Ghostty core is built by Zig in
/// `Debug` when cargo is, text shaping in an unoptimised build is several times slower, and this
/// test runs beside every other test of the crate. It exists to fail a real regression — a
/// per-cell allocation, a full relayout for one echoed character — while the release run asserts
/// the frame budget.
const BUDGET: Duration = Duration::from_millis(16);
const DEV_BUDGET: Duration = Duration::from_millis(250);

#[test]
fn an_echoed_character_commits_inside_one_frame_of_the_display() {
    // A screen with text on every row: a commit re-reads the whole grid, so an empty one would
    // measure less than the window ever has to draw.
    let screen = (0..ROWS - 1)
        .map(|row| format!("perch> command output line {row} with some text on it"))
        .collect::<Vec<_>>()
        .join("\r\n");
    let mut runtime = Runtime::new(
        Shell::idle(Vec::new(), 0).with_frame("t", replay(screen.as_bytes(), COLS, ROWS)),
        view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.frame(0.0);
    for _ in 0..4 {
        if !runtime.frame(0.5) {
            break;
        }
    }

    let mut timings = Vec::with_capacity(KEYS);
    for index in 0..KEYS {
        let line = format!("{screen}\r\nperch> typing a command, key {index}");
        let started = Instant::now();
        runtime.update(|shell, _| {
            shell.set_frame(0, replay(line.as_bytes(), COLS, ROWS));
        });
        runtime.frame(1.0 / 60.0);
        timings.push(started.elapsed());
    }
    timings.sort_unstable();
    let at = |fraction: f64| {
        let index = ((timings.len() - 1) as f64 * fraction).round() as usize;
        timings[index]
    };
    let budget = if cfg!(debug_assertions) {
        DEV_BUDGET
    } else {
        BUDGET
    };
    eprintln!(
        "install frame + view + layout + build over {KEYS} keys at {COLS}x{ROWS}: \
         p50 {:?}, p90 {:?}, max {:?}; budget {budget:?}",
        at(0.5),
        at(0.9),
        at(1.0)
    );
    // The median, not the worst: this runs beside every other test of the crate on a shared
    // machine, so a single descheduled frame says nothing about the dock. A regression that
    // costs per-cell work moves the median, which is what a person typing feels.
    assert!(
        at(0.5) < budget,
        "a key's repaint took {:?} at the median, past {budget:?}",
        at(0.5)
    );
}
