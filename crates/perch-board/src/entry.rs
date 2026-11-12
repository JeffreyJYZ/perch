//! One entry: what a session posted, how it reads back, and who may read it.

use serde::{Deserialize, Serialize};

/// How many paths one entry may name; the rest are dropped rather than refused.
pub const MAX_FILES: usize = 32;

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Monotonic within a board and sortable, so a cursor is one number.
    pub id: u64,
    /// Unix milliseconds.
    pub at: u64,
    /// The session that posted it, which every entry carries.
    pub from: String,
    /// The one session this entry is for; absent means every session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// A label a lead can wait on, such as the wave that is being collected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

/// What a session asked to append. The id and the time belong to the log, not the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Post {
    pub text: String,
    pub to: Option<String>,
    pub group: Option<String>,
    pub files: Vec<String>,
}

impl Entry {
    /// The entry a line holds, or nothing when the line is blank or is not an entry.
    ///
    /// A malformed line is skipped by everyone who reads the log: one bad byte must not make the
    /// board unreadable.
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        serde_json::from_str(line).ok()
    }

    /// The line to append, newline included.
    pub fn line(&self) -> String {
        format!("{}\n", serde_json::to_string(self).unwrap_or_default())
    }

    /// Whether this session may read it: posted by it, addressed to it, or addressed to nobody.
    pub fn visible_to(&self, session: &str) -> bool {
        match &self.to {
            Some(to) => to == session || self.from == session,
            None => true,
        }
    }

    /// Whether the entry carries `needle` anywhere a person would look for it.
    pub fn contains(&self, needle: &str) -> bool {
        let in_field =
            |field: &Option<String>| field.as_deref().is_some_and(|text| text.contains(needle));
        self.text.contains(needle)
            || self.from.contains(needle)
            || in_field(&self.to)
            || in_field(&self.group)
            || self.files.iter().any(|file| file.contains(needle))
    }

    /// The entry as the model reads it: one header line, with long text on the lines under it.
    pub fn render(&self) -> String {
        let mut head = format!("#{} {} {}", self.id, clock(self.at), self.from);
        if let Some(group) = &self.group {
            head.push_str(&format!(" [{group}]"));
        }
        if let Some(to) = &self.to {
            head.push_str(&format!(" to {to}"));
        }
        head.push(':');
        let mut lines = self.text.lines();
        if let Some(first) = lines.next() {
            head.push(' ');
            head.push_str(first.trim_end());
        }
        for line in lines {
            head.push_str("\n    ");
            head.push_str(line.trim_end());
        }
        if !self.files.is_empty() {
            head.push_str("\n    files: ");
            head.push_str(&self.files.join(", "));
        }
        head
    }
}

/// The time of day an entry carries, in UTC, so a board spanning days stays readable.
fn clock(at: u64) -> String {
    let secs = (at / 1000) % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
