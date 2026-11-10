//! A local MCP server: a child process spoken to over its stdin and stdout.

use perch_store::settings::McpServerConfig;
use rmcp::ServiceExt;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};

use crate::{Connected, Error, Result};

/// Spawn the configured command and complete the MCP handshake over its pipes.
///
/// The child's stderr is inherited on purpose: a server that complains about its own
/// configuration prints where a developer running the app can see it, and MCP itself only
/// uses stdout.
pub(crate) async fn connect(config: &McpServerConfig) -> Result<Connected> {
    let command = config.command.as_deref().ok_or_else(|| {
        Error::Config(format!("mcp server {} has no command to run", config.name))
    })?;
    let args = config.args.clone();
    let process = tokio::process::Command::new(command).configure(|cmd| {
        cmd.args(&args);
    });
    let transport = TokioChildProcess::new(process)?;
    let pid = transport.id();
    let service = ().serve(transport).await.map_err(|error| Error::Server {
        server: config.name.clone(),
        message: format!("initialize failed: {error}"),
    })?;
    let peer = service.peer().clone();
    Ok(Connected { peer, service, pid })
}
