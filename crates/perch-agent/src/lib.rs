//! perch-agent: the OpenAI-compatible provider client.
//!
//! `stream` is synchronous: it starts the request on a worker thread and returns an iterator
//! over decoded events, so a caller with no async runtime can consume a live reply.

pub mod ask;
pub mod cancel;
pub mod chunk;
pub mod compact;
pub mod dispatch;
pub mod event;
pub mod instructions;
mod open;
pub mod provider;
mod replies;
mod report;
mod retry;
mod rounds;
pub mod rules;
mod tools;
pub mod turn;

pub use ask::{ASK_USER, AskUser};
pub use cancel::CancelWatch;
pub use chunk::Chunk;
pub use dispatch::{Answer, Decision, Dispatcher, Interactor, ToolReply};
pub use event::{AgentEvent, EndReason, ToolCall, Usage};
pub use instructions::for_worktree;
pub use provider::{
    ChatRequest, ChunkStream, Endpoint, HttpTransport, ListedModel, Message, Model, ModelTransport,
    Role, SecretString, ToolCallRecord, ToolSpec, list_models, list_models_with, stream,
};
pub use rules::{Permission, Rule, Rules};
pub use tools::Tools;
pub use turn::{Agent, AgentConfig, AgentHandle, MAX_ROUNDS};

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
    #[error("{0}")]
    Length(String),
    #[error("config: {0}")]
    Config(String),
    #[error("stream consumer dropped")]
    Closed,
}

/// A perch-agent result.
pub type Result<T> = std::result::Result<T, Error>;
