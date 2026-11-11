//! The OpenAI-compatible streaming chat client.
//!
//! [`stream`] starts one request on its own thread and hands back an iterator over the decoded
//! events, so the caller stays synchronous (the UI thread and the turn loop never await). The
//! request is a blocking one read as a byte stream, which is what makes a synchronous iterator
//! possible without an async runtime in this process.
//!
//! The pieces are separated by what they do: [`message`] is the conversation and the request,
//! [`wire`] is what goes on the wire and what comes back, [`decode`] turns SSE bytes into
//! [`Chunk`]s, and [`send`] is the one blocking request. [`model`] is the live model handle a
//! session changes, and [`models`] is the provider's own listing of what it offers.

mod decode;
mod endpoint;
mod message;
mod model;
mod models;
mod secret;
mod send;
mod stream;
mod wire;

use std::time::Duration;

use crate::{Error, Result};

pub use endpoint::Endpoint;
pub use message::{ChatRequest, Message, Role, ToolCallRecord, ToolSpec};
pub use model::Model;
pub use models::{
    Http as HttpTransport, Transport as ModelTransport, list_models, list_models_with,
};
pub use secret::SecretString;
pub use stream::ChunkStream;

/// The whole-request deadline, body included. A stream that idles beyond this read fails.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Start one streaming chat-completion request and return its decoded events.
///
/// Returns `Err` only for a request that cannot be built or started; anything that goes wrong
/// once the stream is open arrives as the iterator's final `Err`.
pub fn stream(req: &ChatRequest) -> Result<ChunkStream> {
    let url = endpoint(&req.base_url)?;
    if req.model.trim().is_empty() {
        return Err(Error::Config("model is empty".to_string()));
    }
    let body = serde_json::to_vec(&wire::body_for(req))?;
    let api_key = req.api_key.expose_secret().to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("perch-provider".to_string())
        .spawn(move || send::run(&url, &api_key, &body, &tx))?;
    Ok(ChunkStream::new(rx))
}

/// The full URL for `base`, which may or may not already end in the completions path.
fn endpoint(base: &str) -> Result<String> {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(Error::Config("base_url is empty".to_string()));
    }
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err(Error::Config(format!(
            "base_url is not an http url: {base}"
        )));
    }
    if base.ends_with("/chat/completions") {
        Ok(base.to_string())
    } else {
        Ok(format!("{base}/chat/completions"))
    }
}
