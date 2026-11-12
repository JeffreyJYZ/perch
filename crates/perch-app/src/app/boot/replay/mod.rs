//! What a session did before it was closed: its rows for the column, and the record the model's
//! system message carries so a reopened session goes on rather than starting again.
//!
//! The log is the only copy of a conversation, and this reads it: no socket is opened and no
//! request is sent, so a session with a hundred turns in its log costs the provider nothing at
//! open. What lands is bounded — [`MAX_EXCHANGES`] exchanges and [`CONTEXT_BUDGET`] characters —
//! because the record is read at the head of every later request, not just the next one.

use perch_store::{Logged, SessionEvent, SessionLog};
use perch_ui::Transcript;

use crate::app::Result;

mod context;
mod rows;
mod words;

/// How many exchanges a reopened session brings back into its column and its context.
///
/// One exchange is a person's turn and everything the model did answering it: the reply, the calls
/// it made and what they returned. Eight is the budget — enough that "continue" has something to
/// continue, and small enough that the record at the head of every later request is a few pages
/// rather than the whole session.
pub const MAX_EXCHANGES: usize = 8;

/// How many characters of that record ride into the model's context.
///
/// The cap above bounds the count and this bounds the size: one exchange can hold twelve tool
/// rounds, and a long reply or a wide `grep` is what would otherwise make eight of them hundreds
/// of thousands of characters. Eight kilobytes is about two thousand tokens.
pub const CONTEXT_BUDGET: usize = 8 * 1024;

/// One reopened session: the rows the column draws, and what the model is told.
pub struct Replay {
    pub transcript: Transcript,
    history: String,
}

impl Replay {
    /// The session's system message, with its earlier conversation ahead of the next prompt.
    ///
    /// perch-agent seeds a turn's history with its system message and nothing else, so this is the
    /// one place an earlier conversation reaches the model. It is written to be read as history —
    /// what was said, in order — and never to be answered again.
    pub fn seeded(&self, system: String) -> String {
        if self.history.is_empty() {
            return system;
        }
        format!("{system}\n\n{}\n\n{}", words::HEADING, self.history)
    }
}

/// Everything `log` holds, replayed: the last exchanges as rows, and as history.
pub fn read(log: &SessionLog) -> Result<Replay> {
    Ok(of(&log.logged()?))
}

/// The replay of what a log held, without a file: what [`Replay::read`] reads.
fn of(logged: &Logged) -> Replay {
    let exchanges = split(&logged.events);
    let mut blocks: Vec<String> = Vec::new();
    let mut kept = exchanges.len();
    let mut size = 0;
    for exchange in exchanges.iter().rev() {
        if blocks.len() == MAX_EXCHANGES {
            break;
        }
        let block = context::block(exchange);
        // The newest exchange is kept whatever it costs: a session that was one long turn is the
        // session the person means to go on with, and an empty history would say nothing at all.
        if !blocks.is_empty() && size + block.len() > CONTEXT_BUDGET {
            break;
        }
        size += block.len();
        blocks.push(block);
        kept -= 1;
    }
    blocks.reverse();
    let left_out = &exchanges[..kept];
    let mut transcript = Transcript::default();
    if !left_out.is_empty() {
        transcript.note(words::left_out(left_out));
    }
    if logged.skipped > 0 {
        transcript.note(words::skipped(logged.skipped));
    }
    for exchange in &exchanges[kept..] {
        rows::fold(&mut transcript, exchange);
    }
    Replay {
        transcript,
        history: blocks.concat(),
    }
}

/// One exchange per person's turn: the events from a `User` up to the next one.
fn split(events: &[SessionEvent]) -> Vec<&[SessionEvent]> {
    let mut bounds = vec![0];
    bounds.extend(
        events
            .iter()
            .enumerate()
            .filter(|(index, event)| *index > 0 && matches!(event, SessionEvent::User { .. }))
            .map(|(index, _)| index),
    );
    bounds.push(events.len());
    bounds
        .windows(2)
        .filter(|pair| pair[1] > pair[0])
        .map(|pair| &events[pair[0]..pair[1]])
        .collect()
}
