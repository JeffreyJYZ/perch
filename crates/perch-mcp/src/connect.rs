//! Connecting one server: the handshake, its tool list, and the bounded request they share.

use std::sync::Arc;

use perch_store::settings::McpServerConfig;
use perch_tools::ToolSpec;
use rmcp::model::Tool as DiscoveredTool;
use rmcp::service::{Peer, RoleClient, RunningService};
use serde_json::Value;

use crate::host::{McpHost, lock};
use crate::server::Server;
use crate::tools::McpTool;
use crate::{Error, Note, REQUEST_TIMEOUT, Result, separate};

/// Two MCP client halves: a peer to call through, and the running service that owns them.
pub(crate) struct Connected {
    pub(crate) peer: Peer<RoleClient>,
    pub(crate) service: RunningService<RoleClient, ()>,
    pub(crate) pid: Option<u32>,
}

impl McpHost {
    /// Connects one configured server and records it, connected or failed.
    ///
    /// The tools are *not* registered here: [`super::McpHost::reconcile`] fills the registry from
    /// every live server once, so a server that connects late lands beside the ones already there.
    pub(crate) fn connect_one(&self, config: &McpServerConfig) -> Result<()> {
        let (connected, discovered) = self.open(config)?;
        let peer = connected.peer.clone();
        let mut tools = Vec::new();
        for tool in discovered {
            let name = separate(&config.name, &tool.name);
            let spec = ToolSpec {
                name,
                description: tool
                    .description
                    .as_deref()
                    .unwrap_or("tool contributed by an MCP server")
                    .to_string(),
                parameters: Value::Object((*tool.input_schema).clone()),
            };
            tools.push(Arc::new(McpTool {
                server: config.name.clone(),
                tool: tool.name.to_string(),
                spec,
                shared: Arc::clone(&self.shared),
                peer: peer.clone(),
            }));
        }

        let count = tools.len();
        lock(&self.servers).push(Server::connected(
            config,
            connected.peer,
            connected.service,
            connected.pid,
            tools,
        ));
        self.note(Note::Connected {
            server: config.name.clone(),
            tools: count,
        });
        Ok(())
    }

    /// Connect one server and list its tools, both bounded.
    ///
    /// The OAuth flow runs outside the request bound, because its bound is the one a human
    /// needs, not the one a machine needs; everything after it is a request and gets
    /// [`REQUEST_TIMEOUT`].
    fn open(&self, config: &McpServerConfig) -> Result<(Connected, Vec<DiscoveredTool>)> {
        let token = match (&config.command, &config.url) {
            (None, Some(url)) => match crate::oauth::stored_token(&config.name)? {
                Some(token) => Some(token),
                None => Some(
                    self.shared
                        .runtime
                        .block_on(crate::oauth::access_token(url, &config.name))?,
                ),
            },
            _ => None,
        };
        let shared = Arc::clone(&self.shared);
        let name = config.name.clone();
        shared.runtime.block_on(async {
            tokio::time::timeout(REQUEST_TIMEOUT, async {
                let connected = match (&config.command, &config.url) {
                    (Some(_), _) => crate::stdio::connect(config).await?,
                    (None, Some(url)) => crate::remote::connect(&name, url, token).await?,
                    (None, None) => {
                        return Err(Error::Config(format!(
                            "mcp server {} has neither a command nor a url",
                            config.name
                        )));
                    }
                };
                let tools =
                    connected
                        .peer
                        .list_all_tools()
                        .await
                        .map_err(|error| Error::Server {
                            server: name.clone(),
                            message: format!("tools/list failed: {error}"),
                        })?;
                Ok((connected, tools))
            })
            .await
            .map_err(|_| Error::Timeout {
                server: name.clone(),
                after: REQUEST_TIMEOUT,
            })?
        })
    }
}
