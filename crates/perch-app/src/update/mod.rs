//! The update check on its own thread, and the one line the window reads.
//!
//! A check reaches the network and a download waits on a server, so neither can run on the frame
//! thread: a window that froze for the thirty seconds a request may take would look exactly like
//! a window that crashed. The work therefore lives on a thread of its own (the worker in this
//! module), the frame only ever reads the status that thread last published, and [`Status`] turns
//! that status into the single line the column shows.
//!
//! The boundary this module keeps is the crate's: it checks and stages, it never installs. A
//! press downloads the archive, verifies its SHA-256 and says where the file landed; replacing
//! the running bundle stays a step the person takes after perch quits, because the bundle being
//! replaced is the one this process is running from.

mod status;
mod worker;

use std::sync::Arc;

use perch_update::{Http, Transport};

pub use status::Status;
pub(crate) use worker::Worker;

/// The name the update offer is drawn and reported under.
///
/// It can never be a plugin's name: a plugin is named by its wasm file's stem, and a stem cannot
/// contain `/`, so this one names the app's own offer and nothing else.
pub const OWNER: &str = "perch/update";

/// The one control an offer declares: download and verify the archive, never install it.
pub const STAGE: &str = "stage";

/// The directory under the data directory where a staged archive lands.
pub const UPDATES: &str = "updates";

/// The check to run: the manifest to read, and the transport that reads it.
///
/// Built from `settings.toml`, so a URL a person set is the only thing that starts a request.
pub(crate) struct Plan {
    pub(crate) url: String,
    pub(crate) transport: Arc<dyn Transport>,
}

impl Plan {
    /// The check `url` calls for, or `None` when it is empty — the default, and the only thing
    /// that keeps an unconfigured window off the network.
    ///
    /// A stated transport is used as given, which is what lets a test prove the whole path
    /// without a socket. Without one the real client is built, and a client that will not build
    /// is an error rather than a check that silently never runs.
    pub(crate) fn new(
        url: &str,
        stated: Option<Arc<dyn Transport>>,
    ) -> Result<Option<Plan>, perch_update::Error> {
        if url.is_empty() {
            return Ok(None);
        }
        let transport = match stated {
            Some(transport) => transport,
            None => Arc::new(Http::new()?),
        };
        Ok(Some(Plan {
            url: url.to_string(),
            transport,
        }))
    }
}
