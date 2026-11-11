//! The earlier conversation written out for the model: one labelled entry per thing that happened.

use perch_store::SessionEvent;

use crate::session::CANCELLED;

/// How much of one entry rides into the model's memory.
///
/// A reply and a tool result are read to know what was done, not to be quoted back: five hundred
/// characters name the file that was read and the shape of the answer, while the whole of a large
/// `grep` would cost the record its room for everything else.
const ENTRY: usize = 500;

/// One exchange as the lines of the record.
pub(super) fn block(exchange: &[SessionEvent]) -> String {
    let mut text = String::new();
    for event in exchange {
        match event {
            SessionEvent::User { text: said } => line(&mut text, "user", said),
            SessionEvent::Assistant { text: said } => line(&mut text, "assistant", said),
            SessionEvent::ToolCall { name, args, .. } => line(
                &mut text,
                &format!("assistant called {name}"),
                &args.to_string(),
            ),
            SessionEvent::ToolResult { ok, text: out, .. } => {
                let state = if *ok { "ok" } else { "failed" };
                line(&mut text, &format!("tool result ({state})"), out);
            }
            // A cancelled turn is history like any other, and the model is told which one it was:
            // a reply that stops mid-sentence must not read as an answer that finished.
            SessionEvent::TurnEnd { reason, .. } if reason == CANCELLED => {
                text.push_str("(that turn was cancelled)\n");
            }
            SessionEvent::TurnEnd { .. } | SessionEvent::Note { .. } => {}
        }
    }
    text
}

/// One labelled entry, cut to [`ENTRY`] characters.
fn line(into: &mut String, who: &str, words: &str) {
    into.push_str(who);
    into.push_str(": ");
    into.push_str(&cut(words));
    into.push('\n');
}

/// `words` cut to [`ENTRY`] characters, saying how much the cut left behind.
///
/// A continuation keeps its indent, so a reply that held a code block is still readable as the
/// shape it had while every entry stays one entry.
fn cut(words: &str) -> String {
    let total = words.chars().count();
    let mut taken: String = words.chars().take(ENTRY).collect();
    if total > ENTRY {
        taken.push_str(&format!(" ({} more characters)", total - ENTRY));
    }
    taken.replace('\n', "\n  ")
}
