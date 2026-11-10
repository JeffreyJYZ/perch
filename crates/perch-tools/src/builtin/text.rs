//! Text helpers the tools share: a unified diff, a byte budget, and a line count.

use similar::TextDiff;

/// The unified diff the approval view shows, in the same shape the edit result carries.
pub(super) fn unified(path: &str, before: &str, after: &str) -> String {
    TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(path, path)
        .to_string()
}

/// Cuts `text` to `budget` bytes, reporting how much was dropped.
pub(super) fn truncate(text: String, budget: usize) -> String {
    if text.len() <= budget {
        return text;
    }
    let marker = format!("\n[truncated {} bytes]", text.len());
    let mut cut = budget.saturating_sub(marker.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let dropped = text.len() - cut;
    format!("{}\n[truncated {dropped} bytes]", &text[..cut])
}

pub(super) fn line_count(text: &str) -> usize {
    text.lines().count()
}
