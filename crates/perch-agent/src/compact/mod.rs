//! Answering a context-length rejection: a smaller request, not the same one asked again.
//!
//! A provider that refuses a conversation as too long has answered a question about its size.
//! Asking again with the same messages spends the same money for the same refusal, so the oldest
//! exchanges are dropped, the newest are kept, and the request is asked again below the size that
//! was refused. The turn says it did that, because a transcript that loses history silently is a
//! transcript nobody can trust.

use std::sync::mpsc::Sender;

use crate::Error;
use crate::event::AgentEvent;
use crate::open::{Opened, open};
use crate::provider::{ChatRequest, Message, Role};
use crate::turn::AgentConfig;

mod manual;
mod refusal;

pub use manual::now;
pub use refusal::is_context_length;

/// The shares of the refused size a smaller request is asked at, in that order.
pub const STEPS: [f64; 4] = [0.85, 0.70, 0.50, 0.35];

/// The bytes `messages` costs, counting what the model is charged for reading.
fn size(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|message| {
            message.content.as_deref().map_or(0, str::len)
                + message
                    .tool_calls
                    .iter()
                    .map(|call| call.arguments.len())
                    .sum::<usize>()
        })
        .sum()
}

/// A conversation in the three parts shortening treats differently: the system messages, the
/// older exchanges, and the exchange being answered.
fn split(messages: &[Message]) -> (Vec<Message>, Vec<Message>, Vec<Message>) {
    let head = messages
        .iter()
        .take_while(|message| message.role == Role::System);
    let head: Vec<Message> = head.cloned().collect();
    let rest = &messages[head.len()..];
    let Some(held) = rest.iter().rposition(|message| message.role == Role::User) else {
        return (head, Vec::new(), rest.to_vec());
    };
    let (older, tail) = rest.split_at(held);
    (head, older.to_vec(), tail.to_vec())
}

/// A conversation that was shortened: the messages that are left, and how many whole exchanges
/// went with the ones that were dropped.
pub struct Shortened {
    pub messages: Vec<Message>,
    pub exchanges: usize,
}

/// `messages` with the oldest exchanges dropped until it is at most `target` bytes.
///
/// `None` when that would drop nothing the model has not already been sent without: the system
/// messages are kept, because they are what makes the model behave, and the exchange being
/// answered is kept, because it is the question the turn exists to answer.
pub fn shorten(messages: &[Message], target: usize) -> Option<Shortened> {
    let (head, older, tail) = split(messages);
    // One group per exchange, so a dropped turn never leaves its tools answering a call the
    // model can no longer see it making.
    let mut groups: Vec<Vec<Message>> = Vec::new();
    for message in &older {
        if message.role == Role::User || groups.is_empty() {
            groups.push(Vec::new());
        }
        if let Some(group) = groups.last_mut() {
            group.push(message.clone());
        }
    }
    let mut dropped = 0;
    let mut exchanges = 0;
    while size(&head) + size(&groups.concat()) + size(&tail) > target && !groups.is_empty() {
        dropped += groups.remove(0).len();
        exchanges += 1;
    }
    if dropped == 0 {
        return None;
    }
    let mut smaller = head;
    smaller.extend(groups.concat());
    smaller.extend(tail);
    Some(Shortened {
        messages: smaller,
        exchanges,
    })
}

/// What a rejection left to try.
pub(crate) enum Shrunk {
    /// A smaller request is ready to be asked.
    Again,
    /// Nothing left to drop, or every share of the refused size has been tried.
    Spent,
    /// The transcript is gone, so there is nobody left to ask for.
    Gone,
}

/// One rejection's worth of smaller requests.
pub(crate) struct Compact {
    /// The size that was refused, which every share is taken of.
    refused: Option<usize>,
    taken: usize,
    dropped: usize,
}

impl Compact {
    pub fn new() -> Self {
        Compact {
            refused: None,
            taken: 0,
            dropped: 0,
        }
    }

    /// Rebuilds `history` below the next share of the refused size, and says what it dropped.
    pub fn again(&mut self, history: &mut Vec<Message>, events: &Sender<AgentEvent>) -> Shrunk {
        let refused = *self.refused.get_or_insert_with(|| size(history));
        let Some(share) = STEPS.get(self.taken) else {
            return Shrunk::Spent;
        };
        self.taken += 1;
        let target = (refused as f64 * share) as usize;
        let Some(smaller) = shorten(history, target) else {
            return Shrunk::Spent;
        };
        let Shortened {
            messages,
            exchanges,
        } = smaller;
        let dropped = history.len() - messages.len();
        *history = messages;
        self.dropped += dropped;
        if events
            .send(AgentEvent::Compacted { dropped, exchanges })
            .is_err()
        {
            return Shrunk::Gone;
        }
        Shrunk::Again
    }

    /// The ending words when no smaller request fit either.
    pub fn spent(&self) -> Error {
        Error::Length(format!(
            "the conversation was too long for this model ({} bytes), and dropping {} older \
             messages across {} smaller requests did not fit",
            self.refused.unwrap_or_default(),
            self.dropped,
            self.taken
        ))
    }
}

/// Opens the next request for this turn, asking again smaller when the provider refuses its size.
pub(crate) fn open_within(
    config: &AgentConfig,
    history: &mut Vec<Message>,
    events: &Sender<AgentEvent>,
    stopped: &dyn Fn() -> bool,
) -> Opened {
    let mut compact = Compact::new();
    loop {
        let request = ChatRequest {
            base_url: config.endpoint.base_url(),
            api_key: config.endpoint.api_key(),
            model: config.model.current(),
            messages: history.clone(),
            // The snapshot for this request, taken as it starts. A reload that lands while the
            // reply streams changes the next request, never this one.
            tools: config.tools.specs(),
        };
        match open(&request, stopped) {
            Opened::Failed(error) if is_context_length(&error) => {
                match compact.again(history, events) {
                    Shrunk::Again => continue,
                    Shrunk::Gone => return Opened::Cancelled,
                    Shrunk::Spent => return Opened::Failed(compact.spent()),
                }
            }
            other => return other,
        }
    }
}
