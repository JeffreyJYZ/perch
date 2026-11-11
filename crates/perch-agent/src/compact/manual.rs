//! `/compact`: dropping older exchanges now, at the person's ask rather than a provider's refusal.
//!
//! The rule is the refusal path's, with one difference of measure: a refusal is aiming at a size a
//! provider stated, while a manual compaction has no stated target. It halves the part that can be
//! dropped, so repeated requests converge instead of erasing the conversation the first time.

use crate::provider::Message;

use super::{Shortened, shorten, size, split};

/// What one manual compaction left out: messages, and the whole exchanges they belonged to.
pub struct Dropped {
    pub messages: usize,
    pub exchanges: usize,
}

/// Drops about half of the older exchanges, and reports how much was left out.
///
/// `None` when there is no older exchange to drop. The system messages and the exchange being
/// answered always stay: they are what makes the model behave and what the turn exists to answer.
pub fn now(history: &mut Vec<Message>) -> Option<Dropped> {
    let (_, older, _) = split(history);
    let older = size(&older);
    if older == 0 {
        return None;
    }
    // Keeping the newest exchange and the system messages in the total is what makes this a
    // halving of the droppable part rather than of the whole conversation: a long system message
    // must not make the only reachable target the one that drops everything.
    let target = size(history) - older / 2;
    let Shortened {
        messages,
        exchanges,
    } = shorten(history, target)?;
    let dropped = history.len() - messages.len();
    *history = messages;
    Some(Dropped {
        messages: dropped,
        exchanges,
    })
}
