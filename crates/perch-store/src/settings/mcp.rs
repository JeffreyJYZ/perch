//! The MCP servers the settings file names.

use serde::{Deserialize, Serialize};

/// One MCP server, as written in `settings.toml`.
///
/// A server is either local (`command` plus `args`, spoken to over stdio) or remote (`url`,
/// spoken to over streamable HTTP). The `name` is what namespaces its tools, so it is
/// required and is the one thing a user must choose deliberately.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct McpServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
}

impl McpServerConfig {
    /// A local server spawned as a child process and spoken to over its pipes.
    pub fn stdio(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            command: Some(command.into()),
            args,
            url: None,
        }
    }

    /// A remote server reached over streamable HTTP, with OAuth when it asks for it.
    pub fn remote(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: None,
            args: Vec::new(),
            url: Some(url.into()),
        }
    }
}
