//! One decoded piece of a streamed chat completion.

/// A single event from a streamed provider reply.
///
/// Tool-call fragments are passed through untouched: assembling one call out of its fragments
/// is the turn loop's job, so nothing here decides whether that call is complete.
#[derive(Debug, Clone, PartialEq)]
pub enum Chunk {
    /// Assistant text, appended to the open message.
    Text(String),
    /// A tool-call fragment; `index` groups fragments that belong to one call.
    ToolCall {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        args_fragment: String,
    },
    /// Token counts, reported once at the end of a streamed reply.
    Usage { input: u32, output: u32 },
}
