//! A remote MCP server: streamable HTTP with a bearer token.

use rmcp::ServiceExt;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};

use crate::{Connected, Error, REQUEST_TIMEOUT, Result};

/// Open a streamable HTTP session against `url`, carrying `token` as the bearer credential.
///
/// The token is either one this host obtained through [`crate::oauth`] or one it read back
/// from the keychain; when it is `None` the request is anonymous, which is what a server with
/// no authorization requirement wants.
pub(crate) async fn connect(server: &str, url: &str, token: Option<String>) -> Result<Connected> {
    let mut config = StreamableHttpClientTransportConfig::with_uri(url);
    if let Some(token) = token {
        config = config.auth_header(format!("Bearer {token}"));
    }
    // The reqwest client carries the same bound as every other request, so a server that
    // accepts the connection and then goes quiet cannot hold a call open.
    let client = reqwest::Client::builder()
        .connect_timeout(REQUEST_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let transport = StreamableHttpClientTransport::with_client(client, config);
    let service = ().serve(transport).await.map_err(|error| Error::Server {
        server: server.to_string(),
        message: format!("initialize failed: {error}"),
    })?;
    let peer = service.peer().clone();
    Ok(Connected {
        peer,
        service,
        pid: None,
    })
}
