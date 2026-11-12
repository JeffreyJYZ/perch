//! What the app is waiting on, as the window's own word for it.
//!
//! The mapping lives here rather than inside the trait implementation so the word a press reads
//! and the state it reads it from are one short file of their own.

use perch_ui::shell::Waiting;

use crate::Pending;

/// The word for what a session is waiting on, if anything.
pub(crate) fn of(pending: Option<&Pending>) -> Waiting {
    match pending {
        Some(Pending::Approval { .. }) => Waiting::Approval,
        Some(Pending::Question {
            options, multiple, ..
        }) => Waiting::Question {
            options: options.len(),
            multiple: *multiple,
        },
        None => Waiting::Nothing,
    }
}
