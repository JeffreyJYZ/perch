//! What a reopened session says about its own log, and how the record is introduced.

use perch_store::SessionEvent;

/// The line that tells the model what the block below it is.
pub(super) const HEADING: &str = "## Earlier in this session\n\nWhat happened before the app was \
                                   closed, oldest first. Read it as history: do not answer it \
                                   again, and go on from where it stopped.";

/// The note for the exchanges the caps left out, said once at the top of the column.
///
/// The same shape as `/compact`'s note, because it is the same loss: the person sees how many
/// turns are not on screen, and is told where they still are.
pub(super) fn left_out(exchanges: &[&[SessionEvent]]) -> String {
    let messages: usize = exchanges
        .iter()
        .flat_map(|exchange| exchange.iter())
        .filter(|event| carried(event))
        .count();
    let turns = if exchanges.len() == 1 {
        "older exchange"
    } else {
        "older exchanges"
    };
    format!(
        "reopened: {} {turns} ({messages} messages) left out; the log still has them.",
        exchanges.len()
    )
}

/// The note for log lines that could not be read back.
pub(super) fn skipped(lines: usize) -> String {
    let what = if lines == 1 { "line" } else { "lines" };
    let was = if lines == 1 { "was" } else { "were" };
    format!("reopened: {lines} {what} of the log could not be read and {was} skipped.")
}

/// Whether an event is one of the messages a conversation is made of, which is what a person
/// counts when they hear that something was left out.
fn carried(event: &SessionEvent) -> bool {
    matches!(
        event,
        SessionEvent::User { .. }
            | SessionEvent::Assistant { .. }
            | SessionEvent::ToolCall { .. }
            | SessionEvent::ToolResult { .. }
    )
}
