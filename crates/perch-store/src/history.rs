//! The prompt history: every prompt a worktree sent, how often and when, ranked by frecency.
//!
//! One JSON file per worktree, named by a fingerprint of the worktree path, so two worktrees never
//! share a file and a long path is still a short name. The score is uses over the days since the
//! last send, so a prompt used daily outranks one tried once and a month-old one falls behind both.
//!
//! Nothing here reads a clock: a caller passes `now`, so a test orders a history without waiting.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

/// How many prompts one worktree keeps: the rarest and oldest fall off the end.
pub const MAX_ENTRIES: usize = 50;

/// One prompt as the history remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prompt {
    /// The text as it was sent, trimmed.
    pub text: String,
    /// How many times it has been sent.
    pub count: u32,
    /// Unix milliseconds of the last send; no two share one, so the newest wins a tie.
    pub at: u64,
}

impl Prompt {
    /// Uses, decayed by the days since the last one: a repeat today counts in full.
    pub fn score(&self, now: u64) -> f64 {
        const DAY: f64 = 86_400_000.0;
        let days = now.saturating_sub(self.at) as f64 / DAY;
        f64::from(self.count) / (1.0 + days)
    }
}

/// One worktree's prompt history, most frecent first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    entries: Vec<Prompt>,
}

impl History {
    /// An empty history.
    pub const fn new() -> History {
        History {
            entries: Vec::new(),
        }
    }

    /// Every prompt, ranked: index zero is the one Up recalls first.
    pub fn entries(&self) -> &[Prompt] {
        &self.entries
    }

    /// The text at `index`, when the history is that long.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|entry| entry.text.as_str())
    }

    /// How many prompts the history holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been sent from this worktree yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Records `text` as sent at `now`: a repeat raises its count rather than adding a row, the
    /// list is re-ranked, and anything past [`MAX_ENTRIES`] is dropped.
    ///
    /// The stamp is at least a millisecond past every stamp held, so two prompts sent inside one
    /// clock tick still have an order and the newest keeps the top of the walk.
    pub fn record(&mut self, text: &str, now: u64) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let newest = self.entries.iter().map(|entry| entry.at).max();
        let at = newest.map_or(now, |newest| now.max(newest.saturating_add(1)));
        match self.entries.iter_mut().find(|entry| entry.text == text) {
            Some(entry) => {
                entry.count = entry.count.saturating_add(1);
                entry.at = at;
            }
            None => self.entries.push(Prompt {
                text: text.to_owned(),
                count: 1,
                at,
            }),
        }
        self.rank(now);
        self.entries.truncate(MAX_ENTRIES);
    }

    /// The history in `path`, or an empty one when the file is missing or unreadable: a file a
    /// crash left half-written is a convenience lost, not a window that refuses to start.
    pub fn load(path: &Path, now: u64) -> History {
        let entries = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<Prompt>>(&text).ok())
            .unwrap_or_default();
        let mut history = History { entries };
        history.heal(now);
        history
    }

    /// Writes the history to `path`, creating the directory it lives in.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string(&self.entries)?;
        let scratch = path.with_extension("json.tmp");
        std::fs::write(&scratch, text)?;
        std::fs::rename(&scratch, path)?;
        Ok(())
    }

    /// Drops empty rows and merges repeats, so a file edited by hand becomes one row per prompt.
    fn heal(&mut self, now: u64) {
        let entries = std::mem::take(&mut self.entries);
        let mut merged: BTreeMap<&str, Prompt> = BTreeMap::new();
        for entry in &entries {
            let text = entry.text.trim();
            if text.is_empty() {
                continue;
            }
            merged
                .entry(text)
                .and_modify(|kept| {
                    kept.count = kept.count.saturating_add(entry.count);
                    kept.at = kept.at.max(entry.at);
                })
                .or_insert(Prompt {
                    text: text.to_owned(),
                    count: entry.count.max(1),
                    at: entry.at,
                });
        }
        self.entries = merged.into_values().collect();
        self.rank(now);
        self.entries.truncate(MAX_ENTRIES);
    }

    fn rank(&mut self, now: u64) {
        self.entries.sort_by(|a, b| {
            b.score(now)
                .total_cmp(&a.score(now))
                .then(b.at.cmp(&a.at))
                .then_with(|| b.text.cmp(&a.text))
        });
    }
}
