//! One discovered MCP tool, adapted to the registry's [`Tool`] trait.

use std::sync::Arc;

use perch_tools::{Error as ToolError, Result as ToolResult, Tool, ToolCtx, ToolOutput, ToolSpec};
use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::{Peer, RoleClient};
use serde_json::Value;

use crate::{Note, REQUEST_TIMEOUT, Shared};

/// A tool a server listed through `tools/list`.
///
/// Its spec is namespaced, its name on the wire is not: the peer is asked for `echo` while the
/// registry knows it as `fixture__echo`.
pub(crate) struct McpTool {
    pub(crate) server: String,
    pub(crate) tool: String,
    pub(crate) spec: ToolSpec,
    pub(crate) shared: Arc<Shared>,
    pub(crate) peer: Peer<RoleClient>,
}

impl Tool for McpTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    // The user's decision: an MCP tool runs without an approval prompt. The record that makes
    // that safe is the note this call writes, which names the server that ran it.
    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> ToolResult<ToolOutput> {
        let arguments = match args {
            Value::Object(arguments) => arguments.clone(),
            Value::Null => JsonObject::new(),
            other => {
                return Err(ToolError::Message(format!(
                    "{} takes an object of arguments, got {other}",
                    self.spec.name
                )));
            }
        };

        let params = CallToolRequestParams::new(self.tool.clone()).with_arguments(arguments);
        // The request is bounded twice: here, so a server that stops answering costs one call,
        // and again by the runtime, so the transport itself cannot hold the thread.
        let answer = self.shared.runtime.block_on(async {
            tokio::time::timeout(REQUEST_TIMEOUT, self.peer.call_tool(params)).await
        });

        let result = match answer {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                self.shared.note(Note::Call {
                    server: self.server.clone(),
                    tool: self.tool.clone(),
                    ok: false,
                });
                return Err(ToolError::Message(format!(
                    "mcp server {} failed to run {}: {error}",
                    self.server, self.tool
                )));
            }
            Err(_elapsed) => {
                self.shared.note(Note::Call {
                    server: self.server.clone(),
                    tool: self.tool.clone(),
                    ok: false,
                });
                return Err(ToolError::Message(format!(
                    "mcp server {} did not answer {} within {}s",
                    self.server,
                    self.tool,
                    REQUEST_TIMEOUT.as_secs()
                )));
            }
        };

        let ok = result.is_error != Some(true);
        self.shared.note(Note::Call {
            server: self.server.clone(),
            tool: self.tool.clone(),
            ok,
        });

        let mut text = result
            .content
            .iter()
            .filter_map(|block| block.as_text())
            .map(|content| content.text.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        if text.is_empty()
            && let Some(structured) = &result.structured_content
        {
            text = structured.to_string();
        }

        if ok {
            return Ok(ToolOutput::text(text));
        }
        Err(ToolError::Message(if text.is_empty() {
            format!(
                "mcp server {} reported a failure from {}",
                self.server, self.tool
            )
        } else {
            text
        }))
    }
}
