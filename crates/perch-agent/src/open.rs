//! Opening a provider stream, asking again while the provider says "later".

use crate::Chunk;
use crate::provider::ChatRequest;
use crate::retry::{self, Retries};
use crate::stream;

/// The result of asking the provider for a stream, once a first event has arrived.
pub(crate) enum Opened {
    /// The reply is arriving: the first event, then the rest of it.
    Stream(Box<dyn Iterator<Item = crate::Result<Chunk>>>),
    Cancelled,
    /// The provider would not start a reply. The error is kept whole, because a caller has to
    /// tell a rate limit from a refusal it can answer by sending a smaller request.
    Failed(crate::Error),
}

/// Opens a stream, asking again when the provider says "later".
///
/// Only a failure that arrives before the first event is retried: a stream that broke after text
/// arrived has already been shown, and asking again would print the answer twice.
pub(crate) fn open(request: &ChatRequest, stopped: &dyn Fn() -> bool) -> Opened {
    let mut retries = Retries::new();
    loop {
        let mut chunks = match stream(request) {
            Ok(chunks) => chunks,
            Err(error) => match retries.after(&error) {
                Some(gap) => {
                    if !retry::wait(gap, stopped) {
                        return Opened::Cancelled;
                    }
                    continue;
                }
                None => return Opened::Failed(error),
            },
        };
        match chunks.next() {
            Some(Ok(first)) => {
                return Opened::Stream(Box::new(std::iter::once(Ok(first)).chain(chunks)));
            }
            Some(Err(error)) => match retries.after(&error) {
                Some(gap) => {
                    if !retry::wait(gap, stopped) {
                        return Opened::Cancelled;
                    }
                    continue;
                }
                None => return Opened::Failed(error),
            },
            None => {
                let error = crate::Error::Truncated;
                return match retries.after(&error) {
                    Some(gap) => {
                        if !retry::wait(gap, stopped) {
                            return Opened::Cancelled;
                        }
                        continue;
                    }
                    None => Opened::Failed(error),
                };
            }
        }
    }
}
