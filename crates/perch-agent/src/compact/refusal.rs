//! Reading a provider's refusal as a question about the request's size.
//!
//! It is its own decision, never a retryable failure: the size is a property of the request, so the
//! answer is a smaller request.

use crate::Error;

/// How providers word "this conversation is too long".
const WORDINGS: [&str; 4] = [
    "context length",
    "too many tokens",
    "maximum context",
    "prompt is too long",
];

/// Whether `error` is a provider saying the request is too long to accept.
pub fn is_context_length(error: &Error) -> bool {
    let Error::Status { status, body } = error else {
        return false;
    };
    if *status != 400 {
        return false;
    }
    // `context_length_exceeded` is the same refusal spelled as a code.
    let body = body.to_lowercase().replace('_', " ");
    WORDINGS.iter().any(|wording| body.contains(wording))
}
