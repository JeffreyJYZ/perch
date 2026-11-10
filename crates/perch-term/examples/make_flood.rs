//! Generate `tests/fixtures/flood.bin`, the byte stream the flood test replays.
//!
//! Run it once and check the file in:
//!
//! ```sh
//! cargo run -p perch-term --example make_flood
//! ```
//!
//! The stream has to be realistic, not repeated filler: a frame budget measured
//! against a megabyte of one repeated SGR pair would pass on any parser. So it
//! emits what a busy shell actually prints — coloured compiler output, redrawn
//! progress bars, cursor jumps and line clears — with the shape of one of each.

use std::{fs, io::Write, path::PathBuf, time::Instant};

/// Roughly 1.5 MiB, which is a large file `cat`ed into a 120x40 viewport.
const TARGET_BYTES: usize = 1_500_000;

fn main() {
    let started = Instant::now();
    let bytes = generate(TARGET_BYTES);
    let generated = started.elapsed();

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flood.bin");
    fs::create_dir_all(path.parent().expect("the fixture has a directory"))
        .expect("create the fixture directory");
    fs::write(&path, &bytes).expect("write the fixture");

    println!(
        "wrote {} bytes ({} KiB) to {} in {:?}",
        bytes.len(),
        bytes.len() / 1024,
        path.display(),
        generated
    );
}

fn generate(target: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target + 4096);
    let mut rng = Rng::new(0xc0ff_ee12);
    let mut line = 0u32;

    while out.len() < target {
        line += 1;
        // Compiler-shaped output: a status word in a colour, then a path.
        let colour = 31 + (rng.next() % 6);
        write!(
            out,
            "\x1b[{colour}m{:>9}\x1b[0m {:<9} {}\r\n",
            noun(rng.next()),
            rng.next() % 999,
            path_like(&mut rng)
        )
        .expect("write to a vec");

        // A progress bar that repaints itself, which is where a parser meets
        // carriage returns and line clears rather than newlines.
        if line.is_multiple_of(7) {
            for step in 0..12u32 {
                let filled = (step * 3) as usize;
                write!(
                    out,
                    "\r\x1b[2K  {:>3}% [\x1b[32m{}\x1b[0m{}] {}",
                    step * 9 - step,
                    "=".repeat(filled),
                    " ".repeat(36 - filled),
                    noun(u32::from(rng.next() as u16))
                )
                .expect("write to a vec");
            }
            out.extend_from_slice(b"\r\n");
        }

        // A styled paragraph, so the frame carries bold, italic, underline and
        // a truecolor run rather than one palette entry.
        if line.is_multiple_of(5) {
            write!(
                out,
                "  \x1b[1mbold\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \
				 \x1b[38;2;{};{};{}mrgb\x1b[0m \x1b[38;5;{}m256\x1b[0m {}\r\n",
                rng.next() % 256,
                rng.next() % 256,
                rng.next() % 256,
                rng.next() % 256,
                prose(&mut rng)
            )
            .expect("write to a vec");
        }

        // Cursor movement and a box drawn in place, which is what a `top`-like
        // repaint looks like: absolute positioning, not scrollback.
        if line.is_multiple_of(11) {
            let row = 3 + rng.next() % 30;
            let col = 1 + rng.next() % 90;
            write!(out, "\x1b[{row};{col}H\x1b[36m┌────┐\x1b[0m").expect("write to a vec");
            write!(out, "\x1b[{};{}H\x1b[36m│ 日本 │\x1b[0m", row + 1, col)
                .expect("write to a vec");
            write!(out, "\x1b[{};{}H\x1b[36m└────┘\x1b[0m", row + 2, col).expect("write to a vec");
            out.extend_from_slice(b"\x1b[1;1H");
        }

        // The odd carriage-return spinner, so overstrike is exercised.
        if line.is_multiple_of(3) {
            for frame in ["⠋", "⠙", "⠹", "⠸"] {
                write!(out, "\r\x1b[33m{frame}\x1b[0m working").expect("write to a vec");
            }
            out.extend_from_slice(b"\r\x1b[32m\xe2\x9c\x93\x1b[0m done\r\n");
        }
    }

    out
}

fn noun(seed: u32) -> &'static str {
    const WORDS: [&str; 12] = [
        "Compiling",
        "Finished",
        "warning",
        "error",
        "Downloading",
        "Checking",
        "Building",
        "Linking",
        "Running",
        "Documenting",
        "Blocking",
        "Updating",
    ];
    WORDS[(seed as usize) % WORDS.len()]
}

fn path_like(rng: &mut Rng) -> String {
    const CRATES: [&str; 8] = [
        "perch-term",
        "perch-ui",
        "perch-agent",
        "perch-store",
        "perch-tools",
        "perch-files",
        "libghostty-vt",
        "portable-pty",
    ];
    const FILES: [&str; 8] = [
        "src/session.rs",
        "src/frame.rs",
        "src/keys.rs",
        "src/provider.rs",
        "src/registry.rs",
        "tests/replay.rs",
        "src/lib.rs",
        "build.rs",
    ];
    format!(
        "{} v0.1.{} ({}/{})",
        CRATES[(rng.next() as usize) % CRATES.len()],
        rng.next() % 20,
        FILES[(rng.next() as usize) % FILES.len()],
        rng.next() % 4_000
    )
}

fn prose(rng: &mut Rng) -> String {
    const WORDS: [&str; 10] = [
        "the",
        "frame",
        "budget",
        "stays",
        "inside",
        "its",
        "stated",
        "limit",
        "while",
        "streaming",
    ];
    let mut text = String::new();
    for _ in 0..(4 + rng.next() % 10) {
        text.push_str(WORDS[(rng.next() as usize) % WORDS.len()]);
        text.push(' ');
    }
    text.truncate(text.trim_end().len());
    text
}

/// A deterministic xorshift, so regenerating the fixture gives the same bytes.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}
