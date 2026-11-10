//! perch-agent: the OpenAI-compatible provider client.
//!
//! `stream` is synchronous: it starts the request on a worker thread and returns an iterator
//! over decoded events, so a caller with no async runtime can consume a live reply.

pub mod chunk;
pub mod provider;

pub use chunk::Chunk;
pub use provider::{
    ChatRequest, ChunkStream, Message, Role, SecretString, ToolCallRecord, ToolSpec, stream,
};

/// Every failure perch-agent can produce.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("provider returned {status}: {body}")]
    Status { status: u16, body: String },
    #[error("stream ended without [DONE], so the reply is incomplete")]
    Truncated,
    #[error("config: {0}")]
    Config(String),
    #[error("stream consumer dropped")]
    Closed,
}

/// A perch-agent result.
pub type Result<T> = std::result::Result<T, Error>;
