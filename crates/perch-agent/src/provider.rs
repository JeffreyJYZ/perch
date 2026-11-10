//! The OpenAI-compatible streaming chat client.
//!
//! `stream` starts one request on its own thread and hands back an iterator over the decoded
//! events, so the caller stays synchronous (the UI thread and the turn loop never await). The
//! request is a blocking one read as a byte stream, which is what makes a synchronous iterator
//! possible without an async runtime in this process.

use std::fmt;
use std::io::Read;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Chunk, Error, Result};

/// The whole-request deadline, body included. A stream that idles beyond this read fails.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// How much of an error body is kept before truncating it.
const ERROR_BODY_LIMIT: usize = 8 * 1024;

/// Read size for the response body.
const READ_BUFFER: usize = 8 * 1024;

/// A secret that never reaches `Debug`, a log line or a session file.
///
/// A local newtype rather than `secrecy`: the crate is not in the workspace lock, and this is
/// the only behaviour perch needs from it.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// Wrap `secret`.
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// The secret itself. Call it only where the value is used, never where it is printed.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Whether the secret is empty, in which case no authorization header is sent.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for SecretString {
    fn from(secret: &str) -> Self {
        Self(secret.to_string())
    }
}

impl From<String> for SecretString {
    fn from(secret: String) -> Self {
        Self(secret)
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString(<redacted>)")
    }
}

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

/// The decoded events of one reply, in stream order.
///
/// The iterator yields exactly one `Err` when the reply fails, and that error is always its last
/// item: a half-received tool call can never be mistaken for a finished one by a caller that
/// stops at the first error.
pub struct ChunkStream {
    rx: Receiver<Result<Chunk>>,
    finished: bool,
}

impl Iterator for ChunkStream {
    type Item = Result<Chunk>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.rx.recv() {
            Ok(item) => {
                if item.is_err() {
                    self.finished = true;
                }
                Some(item)
            }
            Err(_) => {
                self.finished = true;
                None
            }
        }
    }
}

/// Start one streaming chat-completion request and return its decoded events.
///
/// Returns `Err` only for a request that cannot be built or started; anything that goes wrong
/// once the stream is open arrives as the iterator's final `Err`.
pub fn stream(req: &ChatRequest) -> Result<ChunkStream> {
    let url = endpoint(&req.base_url)?;
    if req.model.trim().is_empty() {
        return Err(Error::Config("model is empty".to_string()));
    }
    let body = serde_json::to_vec(&body_for(req))?;
    let api_key = req.api_key.expose_secret().to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("perch-provider".to_string())
        .spawn(move || run(&url, &api_key, &body, &tx))?;
    Ok(ChunkStream {
        rx,
        finished: false,
    })
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

/// The provider thread: one request, then the decoded body, then either the final `Err` or a
/// closed channel.
fn run(url: &str, api_key: &str, body: &[u8], tx: &Sender<Result<Chunk>>) {
    if let Err(error) = send_and_read(url, api_key, body, tx) {
        let _ = tx.send(Err(error));
    }
}

fn send_and_read(url: &str, api_key: &str, body: &[u8], tx: &Sender<Result<Chunk>>) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let mut request = client
        .post(url)
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .body(body.to_vec());
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }
    let mut response = request.send()?;
    let status = response.status();
    if !status.is_success() {
        let mut text = String::new();
        let _ = response.read_to_string(&mut text);
        return Err(Error::Status {
            status: status.as_u16(),
            body: truncate(text),
        });
    }
    read_events(&mut response, tx)
}

/// Read the body, decode SSE events and forward them until `data: [DONE]`.
fn read_events(response: &mut impl Read, tx: &Sender<Result<Chunk>>) -> Result<()> {
    let mut decoder = Decoder::default();
    let mut buffer = [0u8; READ_BUFFER];
    loop {
        let read = match response.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(Error::Io(error)),
        };
        for event in decoder.feed(&buffer[..read]) {
            if event == "[DONE]" {
                return Ok(());
            }
            dispatch(&event, tx)?;
        }
    }
    // The body ended and the provider never said it was done, so whatever arrived is partial.
    Err(Error::Truncated)
}

/// Split the body into events on blank lines, ignoring comments and every field but `data:`.
#[derive(Default)]
struct Decoder {
    pending: Vec<u8>,
    data: Vec<String>,
}

impl Decoder {
    /// Feed received bytes, returning every event completed by them.
    fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=newline).collect();
            let line = &line[..line.len() - 1];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.join("\n"));
                    self.data.clear();
                }
            } else if let Some(value) = line.strip_prefix(b"data:") {
                let value = value.strip_prefix(b" ").unwrap_or(value);
                self.data.push(String::from_utf8_lossy(value).into_owned());
            }
            // `:comment`, `event:`, `id:` and `retry:` carry nothing the decoder needs.
        }
        events
    }
}

/// Decode one event's data and forward the chunks it contains.
fn dispatch(data: &str, tx: &Sender<Result<Chunk>>) -> Result<()> {
    if data.trim().is_empty() {
        return Ok(());
    }
    let wire: WireChunk = serde_json::from_str(data)?;
    for choice in wire.choices {
        if let Some(content) = choice.delta.content
            && !content.is_empty()
        {
            send(tx, Chunk::Text(content))?;
        }
        for call in choice.delta.tool_calls.into_iter().flatten() {
            let function = call.function.unwrap_or_default();
            send(
                tx,
                Chunk::ToolCall {
                    index: call.index,
                    id: call.id,
                    name: function.name,
                    args_fragment: function.arguments.unwrap_or_default(),
                },
            )?;
        }
    }
    if let Some(usage) = wire.usage {
        send(
            tx,
            Chunk::Usage {
                input: usage.input(),
                output: usage.output(),
            },
        )?;
    }
    Ok(())
}

fn send(tx: &Sender<Result<Chunk>>, chunk: Chunk) -> Result<()> {
    tx.send(Ok(chunk)).map_err(|_| Error::Closed)
}

/// Keep the head of a provider error body; the tail helps nobody and an unbounded body is a
/// memory sink on a hostile server.
fn truncate(text: String) -> String {
    if text.len() <= ERROR_BODY_LIMIT {
        return text;
    }
    let mut cut = ERROR_BODY_LIMIT;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n[truncated {} bytes]", &text[..cut], text.len() - cut)
}

/// The request body: OpenAI chat completions with streaming and usage enabled.
#[derive(Serialize)]
struct WireBody<'a> {
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

fn body_for(req: &ChatRequest) -> WireBody<'_> {
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
struct WireChunk {
    #[serde(default)]
    choices: Vec<WireChoice>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Debug, Deserialize)]
struct WireChoice {
    #[serde(default)]
    delta: WireDelta,
}

#[derive(Debug, Default, Deserialize)]
struct WireDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<WireToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct WireToolCallDelta {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<WireFunctionDelta>,
}

#[derive(Debug, Default, Deserialize)]
struct WireFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// Usage arrives under OpenAI's `prompt_tokens`/`completion_tokens` or, on newer APIs,
/// `input_tokens`/`output_tokens`; both are accepted.
#[derive(Debug, Deserialize)]
struct WireUsage {
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
    fn input(&self) -> u32 {
        self.prompt_tokens.or(self.input_tokens).unwrap_or(0)
    }

    fn output(&self) -> u32 {
        self.completion_tokens.or(self.output_tokens).unwrap_or(0)
    }
}
