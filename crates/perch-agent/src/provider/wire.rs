//! What goes on the wire, and what comes back.

use serde::{Deserialize, Serialize};

use super::message::{ChatRequest, Role};

/// The request body: OpenAI chat completions with streaming and usage enabled.
#[derive(Serialize)]
pub(super) struct WireBody<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<WireTool<'a>>,
    stream: bool,
    stream_options: StreamOptions,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'a Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<WireOutgoingCall<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
}

#[derive(Serialize)]
struct WireOutgoingCall<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireOutgoingFunction<'a>,
}

#[derive(Serialize)]
struct WireOutgoingFunction<'a> {
    name: &'a str,
    arguments: &'a str,
}

#[derive(Serialize)]
struct WireTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireToolFunction<'a>,
}

#[derive(Serialize)]
struct WireToolFunction<'a> {
    name: &'a str,
    description: &'a str,
    parameters: &'a serde_json::Value,
}

pub(super) fn body_for(req: &ChatRequest) -> WireBody<'_> {
    WireBody {
        model: &req.model,
        messages: req
            .messages
            .iter()
            .map(|message| WireMessage {
                role: &message.role,
                content: message.content.as_deref(),
                tool_calls: message
                    .tool_calls
                    .iter()
                    .map(|call| WireOutgoingCall {
                        id: &call.id,
                        kind: "function",
                        function: WireOutgoingFunction {
                            name: &call.name,
                            arguments: &call.arguments,
                        },
                    })
                    .collect(),
                tool_call_id: message.tool_call_id.as_deref(),
            })
            .collect(),
        tools: req
            .tools
            .iter()
            .map(|tool| WireTool {
                kind: "function",
                function: WireToolFunction {
                    name: &tool.name,
                    description: &tool.description,
                    parameters: &tool.parameters,
                },
            })
            .collect(),
        stream: true,
        stream_options: StreamOptions {
            include_usage: true,
        },
    }
}

/// One decoded `data:` payload of a streaming reply.
#[derive(Debug, Deserialize)]
pub(super) struct WireChunk {
    #[serde(default)]
    pub(super) choices: Vec<WireChoice>,
    #[serde(default)]
    pub(super) usage: Option<WireUsage>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WireChoice {
    #[serde(default)]
    pub(super) delta: WireDelta,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct WireDelta {
    #[serde(default)]
    pub(super) content: Option<String>,
    #[serde(default)]
    pub(super) tool_calls: Option<Vec<WireToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WireToolCallDelta {
    #[serde(default)]
    pub(super) index: usize,
    #[serde(default)]
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) function: Option<WireFunctionDelta>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct WireFunctionDelta {
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) arguments: Option<String>,
}

/// Usage arrives under OpenAI's `prompt_tokens`/`completion_tokens` or, on newer APIs,
/// `input_tokens`/`output_tokens`; both are accepted.
#[derive(Debug, Deserialize)]
pub(super) struct WireUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
}

impl WireUsage {
    pub(super) fn input(&self) -> u32 {
        self.prompt_tokens.or(self.input_tokens).unwrap_or(0)
    }

    pub(super) fn output(&self) -> u32 {
        self.completion_tokens.or(self.output_tokens).unwrap_or(0)
    }
}
