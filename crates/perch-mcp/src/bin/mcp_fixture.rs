//! The stdio MCP server the tests spawn.
//!
//! It exists so `perch-mcp`'s stdio transport is exercised against a real process speaking
//! MCP over pipes: the client spawns this binary, initializes, lists one tool, calls it, and
//! the disconnect test kills it with a signal and watches the transport close.
//!
//! It is a `[[bin]]` in this crate, so integration tests find it through
//! `env!("CARGO_BIN_EXE_mcp-fixture")` and no Node or Python runtime is involved.

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, JsonObject,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::transport::stdio;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, model::Tool, serde_json::json,
    service::RequestContext,
};

/// The one tool this server offers. A call echoes back whatever `text` it was given.
const ECHO: &str = "echo";

#[derive(Clone, Copy, Default)]
struct Fixture;

impl ServerHandler for Fixture {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("perch test fixture: one echo tool, no resources, no prompts")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let schema = json!({
            "type": "object",
            "properties": { "text": { "type": "string", "description": "text to echo" } },
            "required": ["text"],
        });
        let input: JsonObject = schema.as_object().cloned().unwrap_or_default();
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            ECHO,
            "Echo the given text back unchanged.",
            input,
        )]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name != ECHO {
            return Err(McpError::invalid_params(
                format!("no tool named {}", request.name),
                None,
            ));
        }
        let text = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("text"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]).into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = Fixture.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
