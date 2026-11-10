//! The conversation the caller builds: roles, messages and the request itself.

use serde::{Deserialize, Serialize};

use super::secret::SecretString;

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// One message in the conversation sent to the provider.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    /// `None` for an assistant turn that only called tools, which is what OpenAI accepts.
    pub content: Option<String>,
    /// Tool calls an assistant turn asked for; empty on every other role.
    pub tool_calls: Vec<ToolCallRecord>,
    /// The call this message answers; only set on a [`Role::Tool`] message.
    pub tool_call_id: Option<String>,
}

impl Message {
    /// A system message.
    pub fn system(text: impl Into<String>) -> Self {
        Self::with_content(Role::System, text)
    }

    /// A user message.
    pub fn user(text: impl Into<String>) -> Self {
        Self::with_content(Role::User, text)
    }

    /// An assistant message that is only text.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::with_content(Role::Assistant, text)
    }

    /// An assistant turn that called tools, optionally with text before the calls.
    pub fn assistant_with_tools(text: Option<String>, tool_calls: Vec<ToolCallRecord>) -> Self {
        Self {
            role: Role::Assistant,
            content: text,
            tool_calls,
            tool_call_id: None,
        }
    }

    /// The result of one tool call, addressed back to the assistant.
    pub fn tool_result(tool_call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(text.into()),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    fn with_content(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: Some(text.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }
}

/// A tool call an assistant turn made, kept so the next request can send it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallRecord {
    pub id: String,
    pub name: String,
    /// The arguments exactly as the model produced them, as JSON text.
    pub arguments: String,
}

/// One tool offered to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// A JSON Schema object describing the arguments.
    pub parameters: serde_json::Value,
}

/// Everything one streaming request needs.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// The API root, e.g. `https://openrouter.ai/api/v1`. `/chat/completions` is appended.
    pub base_url: String,
    pub api_key: SecretString,
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
}
