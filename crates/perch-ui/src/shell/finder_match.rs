//! How one path places against what was typed: the fuzzy order the finder rows are ranked by.
//!
//! Matching is a subsequence over the whole path, so the letters may be spread across folders and
//! still find the file. A needle of five or more may drop one of its letters — that is how a typo
//! still finds the file — and anything shorter must be placed exactly, so two or three letters
//! never match a path at random.

/// Where one path placed against the needle: the fewer letters it had to drop the better, and a
/// file name starting with the needle beats one that only contains it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Hit {
    /// Letters of the needle the path could not place: 0 is an exact subsequence.
    pub(super) missed: usize,
    /// 0 when the file's own name starts with the needle, 1 otherwise.
    pub(super) prefix: u8,
}

/// How `shown` places against `needle`, both already lowercase, or `None` when it does not match.
pub(super) fn hit(shown: &str, needle: &str) -> Option<Hit> {
    let prefix = prefix_rank(shown, needle);
    if needle.is_empty() || places(shown, needle, None) {
        return Some(Hit { missed: 0, prefix });
    }
    if needle.chars().count() < 5 {
        return None;
    }
    (0..needle.chars().count())
        .any(|dropped| places(shown, needle, Some(dropped)))
        .then_some(Hit { missed: 1, prefix })
}

/// 0 when the file's own name starts with `needle`, 1 otherwise.
fn prefix_rank(shown: &str, needle: &str) -> u8 {
    let name = shown.rsplit('/').next().unwrap_or(shown);
    u8::from(!name.starts_with(needle))
}

/// Whether `needle` is a subsequence of `shown`, with the letter at `dropped` ignored.
fn places(shown: &str, needle: &str, dropped: Option<usize>) -> bool {
    let mut needle = needle
        .chars()
        .enumerate()
        .filter(|(at, _)| Some(*at) != dropped);
    let mut wanted = needle.next().map(|(_, ch)| ch);
    for ch in shown.chars() {
        if wanted == Some(ch) {
            wanted = needle.next().map(|(_, ch)| ch);
            if wanted.is_none() {
                return true;
            }
        }
    }
    wanted.is_none()
}
