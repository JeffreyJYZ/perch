//! The words a tool card and a turn's report are written in: the collapsed cap, the whole
//! output, and the one line a finished turn ends with.
//!
//! These are text, not drawing, so they are their own module: the column renders them and a test
//! asserts them without a runtime.

/// How many lines of a tool's output a collapsed card shows before it says how many are left.
pub const COLLAPSED_LINES: usize = 5;

/// The line a collapsed card ends with: how many lines it hid, and how many it showed.
///
/// One source for these words, so the cap cannot read two ways in two places.
#[must_use]
pub fn cap_line(hidden: usize) -> String {
    format!("… {hidden} more lines (showing first {COLLAPSED_LINES})")
}

/// A tool's output as a collapsed card shows it: the first [`COLLAPSED_LINES`] lines, then one
/// line naming how many are left and how many were shown. Output inside the cap is returned
/// unchanged, so a short result claims nothing.
#[must_use]
pub fn collapsed_output(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= COLLAPSED_LINES {
        return output.trim_end().to_string();
    }
    let hidden = lines.len() - COLLAPSED_LINES;
    format!(
        "{}\n{}",
        lines[..COLLAPSED_LINES].join("\n"),
        cap_line(hidden)
    )
}

/// A tool's output as an expanded card shows it: every line, and no cap line.
///
/// Trailing whitespace goes the same way [`collapsed_output`] sends it, so the two differ by
/// exactly the lines the cap hid and the cap line itself.
#[must_use]
pub fn expanded_output(output: &str) -> String {
    output.trim_end().to_string()
}

/// The one line a turn's summary reads: what ran, what failed, and the tokens when the provider
/// reported them. Every number carries its label, so a count is never a bare digit.
#[must_use]
pub fn summary_line(tools: usize, failed: usize, tokens: Option<u32>) -> String {
    let ran = match tools {
        1 => "1 tool call ran".to_string(),
        count => format!("{count} tool calls ran"),
    };
    let mut line = match failed {
        0 => format!("{ran}, none failed"),
        1 => format!("{ran}, 1 failed"),
        count => format!("{ran}, {count} failed"),
    };
    if let Some(tokens) = tokens {
        line.push_str(&format!(" · {tokens} tokens"));
    }
    line.push('.');
    line
}
