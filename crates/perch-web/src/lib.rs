//! perch-web: the two tools that reach the network, `webfetch` and `websearch`.
//!
//! Both are read-only and both answer in text the model can read: `webfetch` reduces one page to
//! markdown or plain text, and `websearch` returns titles, URLs and snippets. Neither needs an API
//! key, and neither writes the query anywhere.
//!
//! The HTTP call sits behind [`Fetcher`], so a test never touches the network and the two tools
//! never grow a second transport.

mod fetch;
mod html;
mod net;
mod search;

use std::sync::Arc;
use std::time::Duration;

use perch_tools::Tool;

pub use fetch::{FETCH_MAX_BYTES, WebFetch};
pub use net::{FetchError, Fetcher, Http, Response};
pub use search::{SEARCH_COUNT_DEFAULT, SEARCH_COUNT_MAX, WebSearch};

/// A network call is abandoned after this long.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// Both tools, in the order the model sees them.
pub fn tools() -> Vec<Arc<dyn Tool>> {
    vec![Arc::new(WebFetch::new()), Arc::new(WebSearch::new())]
}

/// The first `limit` bytes of `text`, never inside a UTF-8 sequence.
pub(crate) fn leading(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut cut = limit;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}
