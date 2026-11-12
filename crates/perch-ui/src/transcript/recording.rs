//! What a recorded tool output says about itself: how much of it there is, and whether the tool cut
//! it before the text reached the transcript.
//!
//! A tool that truncates leaves its own line behind — `perch-tools` writes one in `text.rs`,
//! `read.rs`, `list.rs` and `glob.rs` — and that line is the only evidence a reopened session still
//! has. Nothing is inferred past it: the card quotes the line and states the size of the block it
//! holds, so a short block is never drawn as if it were the whole answer.

/// A recorded output as a card can describe it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// Lines in the recorded text.
    pub lines: usize,
    /// Bytes recorded, which is the size of the block the card holds.
    pub bytes: usize,
    /// The line the tool left when it cut the text, trimmed.
    pub cut: Option<String>,
}

/// What `output` says about itself.
#[must_use]
pub fn recorded(output: &str) -> Recorded {
    let text = output.trim_end();
    Recorded {
        lines: text.lines().count(),
        bytes: text.len(),
        cut: text
            .lines()
            .map(str::trim)
            .find(|line| is_cut(line))
            .map(str::to_string),
    }
}

/// Whether `line` is the marker a truncating tool appends or prepends.
///
/// One shape, because the tools share it: `[truncated …]` alone on its line.
fn is_cut(line: &str) -> bool {
    line.starts_with("[truncated") && line.ends_with(']')
}

impl Recorded {
    /// The size, as the opened card's header says it: how many lines, and how many bytes.
    #[must_use]
    pub fn words(&self) -> String {
        format!("{} · {}", lines_word(self.lines), size(self.bytes))
    }

    /// The sentence for a text the tool cut before recording it; `None` when it cut nothing.
    ///
    /// The marker is quoted rather than summarised, and the size that survived is stated: a reader
    /// must be able to tell a whole answer from what the tool managed to keep.
    #[must_use]
    pub fn cut_words(&self) -> Option<String> {
        let cut = self.cut.as_deref()?;
        Some(format!(
            "the tool cut this when it ran ({cut}): {} kept",
            size(self.bytes)
        ))
    }
}

/// A byte count as a person reads it.
#[must_use]
pub fn size(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    let value = bytes as f64;
    if value < KIB {
        return match bytes {
            1 => "1 byte".to_string(),
            count => format!("{count} bytes"),
        };
    }
    if value < KIB * KIB {
        return format!("{:.1} KiB", value / KIB);
    }
    format!("{:.1} MiB", value / (KIB * KIB))
}

/// A line count with its noun agreeing.
#[must_use]
pub fn lines_word(lines: usize) -> String {
    match lines {
        1 => "1 line".to_string(),
        count => format!("{count} lines"),
    }
}
