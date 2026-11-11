//! How a stopped execution is named, and how a fault is drawn against the line it happened on.

use std::time::Duration;

/// Which budget ended an execution, and what it reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// The clock is checked around every host call; this is that check firing.
    Clock {
        after: Duration,
        cap: Duration,
    },
    /// The loop allowance injected through boa's runtime limits, which is how the clock binds to a
    /// program that computes without ever reaching the host. boa 0.22 has no clock hook, so a pure
    /// loop is counted rather than timed.
    Steps {
        steps: u64,
        cap: u64,
        wall: Duration,
    },
    Calls {
        calls: u32,
        cap: u32,
    },
    Output {
        bytes: usize,
        cap: usize,
    },
}

impl Hit {
    /// One plain sentence naming the budget and the count it reached.
    pub fn words(&self) -> String {
        match *self {
            Self::Clock { after, cap } => format!(
                "the wall-clock budget was hit: {:.1} s elapsed (cap {} s)",
                after.as_secs_f64(),
                cap.as_secs_f64()
            ),
            Self::Steps { steps, cap, wall } => format!(
                "the wall-clock budget was hit: a loop ran {steps} iterations without calling a tool, past its {cap}-iteration allowance (the stand-in for {} s of clock)",
                wall.as_secs_f64()
            ),
            Self::Calls { calls, cap } => {
                format!("the tool-call budget was hit: {calls} calls (cap {cap})")
            }
            Self::Output { bytes, cap } => format!(
                "the output budget was hit: {bytes} bytes (cap {cap}); the output was truncated"
            ),
        }
    }
}

/// Why an execution ended before its program finished. Every variant is data: the tool answers
/// with it and the process keeps running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// The program wanted more than one execution may spend.
    Hit(Hit),
    /// The person in front of the window stopped the turn this ran in.
    Cancelled,
}

impl Stop {
    pub fn words(&self) -> String {
        match self {
            Self::Hit(hit) => hit.words(),
            Self::Cancelled => String::from("the execution was cancelled before it finished"),
        }
    }
}

/// A fault drawn against the line it happened on, with a caret under the place.
pub fn render(source: &str, line: usize, column: usize, message: &str) -> String {
    let text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let gutter = format!("{line:>4}");
    let blank = " ".repeat(gutter.chars().count());
    let pad = " ".repeat(column.saturating_sub(1));
    format!("line {line}, column {column}: {message}\n{gutter} | {text}\n{blank} | {pad}^")
}
