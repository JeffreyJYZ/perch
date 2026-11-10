//! Shared helpers for the integration tests. Not a test binary itself: only `.rs` files
//! directly under `tests/` are compiled as their own crate, and each of those uses a subset
//! of these helpers.

#![allow(dead_code)]

use perch_store::settings::McpServerConfig;

/// The fixture server, named `fixture`.
pub fn fixture_server() -> McpServerConfig {
    fixture_server_named("fixture")
}

/// The fixture server under a chosen name, so the namespacing test can try to shadow a
/// builtin by calling itself `bash`.
pub fn fixture_server_named(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        command: Some(env!("CARGO_BIN_EXE_mcp-fixture").to_string()),
        args: Vec::new(),
        url: None,
    }
}
