//! The one request a title costs: which provider and model it asks, and the exact words it sends.
//!
//! It runs on its own thread and reads only the answer: thinking, tool fragments and usage are
//! not a title. The wire shape this client builds carries no temperature field, so the system
//! line stands in for a low one — answer only, no preamble, no quotes.

use perch_agent::{ChatRequest, Message, SecretString};

use super::ProviderDefaults;
use crate::session::SessionRuntime;

/// The exact system line the title request sends.
pub const TITLE_SYSTEM: &str = "You name conversations. Answer with a title of at most six words, \
     in the language of the question. No quotes, no trailing punctuation, no prefix like \
     \"Title:\". Answer with the title only.";

/// One title request, read to its end on the caller's thread: the caller is already off the frame.
pub(super) fn ask(request: &ChatRequest) -> Result<String, String> {
    let mut text = String::new();
    for chunk in perch_agent::stream(request).map_err(|error| error.to_string())? {
        if let perch_agent::Chunk::Text(part) = chunk.map_err(|error| error.to_string())? {
            text.push_str(&part);
        }
    }
    Ok(text)
}

/// The request for one exchange, on the session's own provider and model.
pub(super) fn request(
    runtime: &SessionRuntime,
    defaults: &ProviderDefaults,
) -> Option<ChatRequest> {
    let question = first_user(runtime)?;
    let answer = first_reply(runtime)?;
    let base_url = runtime.endpoint.base_url();
    let base_url = if base_url.trim().is_empty() {
        defaults.base_url.clone()
    } else {
        base_url
    };
    let key = runtime.endpoint.api_key();
    let api_key = if key.is_empty() {
        SecretString::from(defaults.api_key.clone())
    } else {
        key
    };
    let model = runtime.model.current();
    if base_url.trim().is_empty() || model.trim().is_empty() {
        return None;
    }
    Some(ChatRequest {
        base_url,
        api_key,
        model,
        messages: vec![
            Message::system(TITLE_SYSTEM),
            Message::user(format!(
                "Question:\n{question}\n\nAnswer:\n{answer}\n\nWrite the title now."
            )),
        ],
        tools: Vec::new(),
    })
}

/// The first thing the person asked.
fn first_user(runtime: &SessionRuntime) -> Option<String> {
    runtime
        .transcript
        .items()
        .iter()
        .find_map(|item| match item {
            perch_ui::Item::User { text } if !text.trim().is_empty() => Some(text.clone()),
            _ => None,
        })
}

/// The first thing the model answered.
fn first_reply(runtime: &SessionRuntime) -> Option<String> {
    runtime
        .transcript
        .items()
        .iter()
        .find_map(|item| match item {
            perch_ui::Item::Assistant { text } if !text.trim().is_empty() => Some(text.clone()),
            _ => None,
        })
}
