//! The saved permission rules: one JSON rule per line in `permissions.jsonl`.

use std::path::Path;

use perch_agent::Rules;
use perch_store::settings::rules_path;

/// The rules saved beside the sessions, and one line for the column when they could not be read.
///
/// A file that will not parse is never "allow everything" and never a window that will not open:
/// the app starts with no rules at all — so every call that needs permission asks — and the line
/// says why a saved allow did not apply.
pub(super) fn load(dir: &Path) -> (Rules, Option<String>) {
    match Rules::load(rules_path(dir)) {
        Ok(rules) => (rules, None),
        Err(error) => (
            Rules::default(),
            Some(format!("saved permissions were not read: {error}")),
        ),
    }
}
