//! The context meter beside the composer: how much of the model's window the last turn used.
//!
//! One number from one place. The tokens are what the provider reported for the last turn, the
//! window is what it stated for the model in its own listing, and the share is the one division
//! between them. A provider that states no window leaves the share out rather than inventing a
//! denominator, and a session that has not run a turn shows nothing rather than `0%`.

use rune::{El, row, text};
use rune_kit::Kit;

use super::live::Live;
use super::state::Shell;

/// The window's own hint, so the command is found by reading rather than by being told.
const HINT: &str = "/compact compacts the conversation";

/// A token count as the meter writes it: one decimal of a thousand, or of a million.
///
/// The thresholds are OpenCode's, so the same conversation reads the same in both harnesses: the
/// million cut-off exists because 999,950 would otherwise round to `1000.0K`.
pub fn token_words(tokens: u32) -> String {
    if tokens >= 999_950 {
        format!("{:.1}M", f64::from(tokens) / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", f64::from(tokens) / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// `12.3K (6%) used`, or `12.3K used` when the provider states no window.
///
/// `None` before the first turn: nothing has been measured, and a meter that says `0%` claims an
/// observation nobody made.
pub fn context_words(tokens: Option<u32>, limit: Option<u32>) -> Option<String> {
    let tokens = tokens?;
    let used = token_words(tokens);
    let Some(limit) = limit.filter(|limit| *limit > 0) else {
        return Some(format!("{used} used"));
    };
    let percent = (f64::from(tokens) / f64::from(limit) * 100.0).round() as u32;
    Some(format!("{used} ({percent}%) used"))
}

/// The meter and the hint, as the composer draws them.
pub(super) fn composer_meter<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let mut line = row().key("context-meter").w_full().gap(8.0).center_y();
    let words = context_words(shell.tokens_used(), shell.model_limit());
    if let Some(words) = words {
        line = line.child(
            text(words)
                .key("context-meter-value")
                .font(kit.ty.small)
                .color(kit.c.text_faint),
        );
    }
    line.child(
        text(HINT)
            .key("context-meter-hint")
            .font(kit.ty.small)
            .color(kit.c.text_faint),
    )
}

impl<L: Live> Shell<L> {
    /// The tokens the last turn reported, read from the app behind the window.
    pub fn tokens_used(&self) -> Option<u32> {
        self.live.as_ref().and_then(Live::tokens_used)
    }

    /// The context window the provider states for this session's model, when it states one.
    pub fn model_limit(&self) -> Option<u32> {
        self.live.as_ref().and_then(Live::model_limit)
    }
}
