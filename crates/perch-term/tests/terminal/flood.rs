//! The frame budget: a recorded flood must not blow it.

use std::time::{Duration, Instant};

use perch_term::replay_stages;

/// The frame budget the plan states, for the build that ships.
const BUDGET: Duration = Duration::from_millis(150);

/// The same budget for a cargo dev profile.
///
/// `libghostty-vt-sys` builds the vendored Ghostty core with Zig and picks
/// Zig's `Debug` mode whenever cargo is in the dev profile, which is roughly a
/// thousand times slower than `ReleaseFast` on the same bytes. The measured
/// dev-profile replay is about 10.5 s, so this ceiling is not the acceptance
/// number — it exists to fail a real regression (a quadratic parse, a leak)
/// while the release run asserts the 150 ms the plan asks for.
const DEV_BUDGET: Duration = Duration::from_secs(15);

#[test]
fn flood_keeps_frame_build_under_budget() {
    let path = "tests/fixtures/flood.bin";
    let bytes = std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "read {path}: {error}; generate it with `cargo run -p perch-term --example make_flood`"
        )
    });
    let total_budget = if cfg!(debug_assertions) {
        DEV_BUDGET
    } else {
        BUDGET
    };

    let start = Instant::now();
    let (frame, stages) = replay_stages(&bytes, 120, 40);
    let elapsed = start.elapsed();

    eprintln!(
        "flood: {} bytes ({} KiB) replayed into a {}x{} frame: feed {:?}, build {:?}, \
         total {:?}; budget {:?} (frame build {:?})",
        bytes.len(),
        bytes.len() / 1024,
        frame.cols,
        frame.height,
        stages.feed,
        stages.build,
        elapsed,
        total_budget,
        BUDGET
    );

    assert_eq!(frame.rows.len(), 40, "one row per viewport line");
    assert_eq!(frame.rows[0].cells.len(), 120, "one cell per column");
    assert!(
        stages.build < BUDGET,
        "frame build took {:?}, budget {BUDGET:?}",
        stages.build
    );
    assert!(
        elapsed < total_budget,
        "flood took {elapsed:?}, budget {total_budget:?}"
    );
}
