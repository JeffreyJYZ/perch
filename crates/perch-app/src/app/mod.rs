//! The app's session state: sessions read from their logs, one live agent each, and the
//! transcript, tools and dispatch thread those sessions answer with.
//!
//! [`AppState`] is the window's whole model of a live conversation. It owns the turn's events
//! only through [`AppState::pump`], which folds whatever the dispatch thread posted into the
//! active transcript — so the thread that draws is the thread that mutates, and a turn's arrival
//! can never race a frame. Everything a turn needs from a person blocks that dispatch thread and
//! nothing else; the answer comes back through [`AppState::answer`].
//!
//! The tools are the one part that is not fixed when the app opens. [`Worker`] rebuilds them from
//! the plugin directory and the configured MCP servers on its own thread, and the frame only reads
//! the snapshot it publishes: the live registry every dispatch reads, the panels and the words. A
//! change therefore lands at the next call, without a turn being rewound and without a frame
//! waiting for a server to answer.
//!
//! The state is split by the moment it belongs to: `open` and `boot` create sessions,
//! `sessions` switches and reads them, `config` follows the settings file, and `answers`
//! carries what a person replies into a running turn.

mod answers;
mod boot;
mod commands;
mod config;
mod models;
mod open;
mod sessions;
mod title;
mod title_ask;
mod title_words;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use perch_agent::ModelTransport;
use perch_store::secret::SecretStore;
use perch_update::Transport;

use crate::live_registry::Live;
use crate::session::SessionRuntime;
use crate::update::{Status, Worker as UpdateWorker};
use crate::watch::Watch;
use crate::worker::{Snapshot, Worker};

pub use crate::session::CANCELLED;

/// How long one pump waits for the next event before calling the session quiet.
///
/// Short enough that a frame is never held, long enough that a stream's deltas arrive in batches.
pub const QUIET: Duration = Duration::from_millis(25);

/// The system message every session starts from.
const SYSTEM: &str = "You are perch, a coding agent working in the session's worktree. Use the \
                      tools you are given; every path is relative to that worktree.";

/// Everything perch-app can fail at.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("store: {0}")]
    Store(#[from] perch_store::Error),
    #[error("mcp: {0}")]
    Mcp(#[from] perch_mcp::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("no session is open")]
    NoSession,
    #[error("{0}")]
    Model(String),
    #[error(
        "no model is chosen for this session, so nothing was sent: pick one in the composer or Settings"
    )]
    NoModel,
    #[error("session {0} is not open")]
    MissingSession(usize),
    #[error("the worktree {0} no longer exists, so this session cannot run a turn")]
    MissingWorktree(PathBuf),
    #[error("{0}")]
    Folder(String),
    #[error("update: {0}")]
    Update(#[from] perch_update::Error),
}

/// A perch-app result.
pub type Result<T> = std::result::Result<T, Error>;

/// The provider a new session talks to.
///
/// Plain values rather than the whole [`perch_store::Settings`]: a test hands an app a fixture
/// server's base URL without touching the keychain, and the window hands it what the settings file
/// said.
#[derive(Debug, Clone, Default)]
pub struct ProviderDefaults {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// Every open session, which one the window shows, and the tools they share.
pub struct AppState {
    pub(crate) dir: PathBuf,
    pub(crate) sessions: Vec<SessionRuntime>,
    pub(crate) active: usize,
    /// The tools every session's model may call, swapped as a whole when the sources change.
    pub(crate) live: Live,
    /// The registry built off the frame thread, and the panels and words that came with it.
    pub(crate) worker: Worker,
    /// The snapshot the frame last absorbed: what it draws and what every call dispatches through.
    pub(crate) snapshot: Arc<Snapshot>,
    /// The debounced filesystem watcher, when the platform gave one.
    pub(crate) watch: Option<Watch>,
    /// The update check on its own thread, when `settings.toml` names a manifest.
    pub(crate) update: Option<UpdateWorker>,
    /// The manifest URL that worker was started for, so a save can tell whether it changed.
    pub(crate) update_url: String,
    /// The transport every update worker is built with: the real client, or a test's table.
    pub(crate) update_transport: Option<Arc<dyn Transport>>,
    /// The transport the provider's model list is fetched through: the real client, or a test's.
    pub(crate) listings: Arc<dyn ModelTransport>,
    /// The context window each listed model states, by id: what the meter divides by.
    ///
    /// A provider need not state one, and the map simply has no entry for it; the meter then
    /// shows the token count with no share rather than inventing a denominator.
    pub(crate) limits: std::collections::HashMap<String, u32>,
    /// The listing on its way, when one is: the thread that fetched it posts one answer and ends.
    ///
    /// The panel opens on the click and takes the answer on a later frame, so the tab appears at
    /// once instead of after a network round trip.
    pub(crate) models_job: Option<
        std::sync::mpsc::Receiver<std::result::Result<Vec<perch_agent::ListedModel>, String>>,
    >,
    /// The status the frame last read, and therefore the line the column shows.
    pub(crate) update_status: Option<Arc<Status>>,
    defaults: ProviderDefaults,
    /// Whether the provider was stated by the caller rather than read from settings. A stated one
    /// is never overwritten by a reload, which is what lets a fixture server stay pinned.
    stated: bool,
    /// Where the provider key is kept: the login keychain, or a test's in-memory store.
    secrets: Box<dyn SecretStore>,
    /// The session titles on their way, and the sessions already asked about.
    pub(crate) titles: title::Titles,
}
