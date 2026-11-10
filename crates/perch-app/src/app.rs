//! The app's session state: sessions read from their logs, one live agent each, and the
//! transcript, tools and dispatch thread those sessions answer with.
//!
//! [`AppState`] is the window's whole model of a live conversation. It owns the turn's events
//! only through [`AppState::pump`], which folds whatever the dispatch thread posted into the
//! active transcript — so the thread that draws is the thread that mutates, and a turn's arrival
//! can never race a frame. Everything a turn needs from a person blocks that dispatch thread and
//! nothing else; the answer comes back through [`AppState::answer`].
//!
//! The tools are the one part that is not fixed when the app opens. [`Sources`] rebuilds them from
//! the plugin directory and the configured MCP servers, [`AppState::reload`] swaps the result into
//! the live registry, and every dispatch reads the registry when its call starts — so a change
//! lands at the next call without a turn being rewound.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use perch_agent::{Agent, AgentConfig, AgentEvent, Answer, Decision, Dispatcher, SecretString};
use perch_plugins::PanelEntry;
use perch_store::Settings;
use perch_store::secret::{self, SecretStore};
use perch_store::settings::{self as settings_file, McpServerConfig};
use perch_store::{SessionLog, SessionMeta};
use perch_tools::{ToolCtx, ToolOutput, ToolSpec};
use perch_ui::Transcript;
use perch_ui::settings::SettingsDraft;
use perch_ui::shell::{KeyStatus, Panels, SettingsSnapshot};

use crate::bridge;
use crate::live_registry::Live;
use crate::session::SessionRuntime;
use crate::sources::Sources;
use crate::turn::{Pending, Pumped};
use crate::watch::{self, Watch};

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
    #[error("session {0} is not open")]
    MissingSession(usize),
    #[error("the worktree {0} no longer exists, so this session cannot run a turn")]
    MissingWorktree(PathBuf),
}

/// A perch-app result.
pub type Result<T> = std::result::Result<T, Error>;

/// The provider a new session talks to.
///
/// Plain values rather than the whole [`Settings`]: a test hands an app a fixture server's base
/// URL without touching the keychain, and the window hands it what the settings file said.
#[derive(Debug, Clone, Default)]
pub struct ProviderDefaults {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// Every open session, which one the window shows, and the tools they share.
pub struct AppState {
    dir: PathBuf,
    sessions: Vec<SessionRuntime>,
    active: usize,
    /// The tools every session's model may call, swapped as a whole when the sources change.
    live: Live,
    /// What a registry is built from: the plugin directory and the MCP servers.
    sources: Sources,
    /// The debounced filesystem watcher, when the platform gave one.
    watch: Option<Watch>,
    defaults: ProviderDefaults,
    /// Whether the provider was stated by the caller rather than read from settings. A stated one
    /// is never overwritten by a reload, which is what lets a fixture server stay pinned.
    stated: bool,
    /// Where the provider key is kept: the login keychain, or a test's in-memory store.
    secrets: Box<dyn SecretStore>,
}

impl AppState {
    /// Opens every session whose log is in `dir`, reading the provider from settings and the key
    /// from the keychain.
    ///
    /// A keychain that refuses is not fatal: the window opens and a turn reports the provider's
    /// rejection, which is a better failure than a window that will not start.
    pub fn open(dir: &Path) -> Result<AppState> {
        crate::settings::open(dir, None, Box::new(secret::Keychain))
    }

    /// The same, with the provider stated instead of read: what a test or a scripted run uses.
    ///
    /// MCP servers still come from settings and plugins from `<dir>/plugins`, so a session started
    /// this way sees the tools the window would give it.
    pub fn open_with(dir: &Path, defaults: ProviderDefaults) -> Result<AppState> {
        crate::settings::open(dir, Some(defaults), Box::new(secret::Keychain))
    }

    /// The same, with the secret store stated: what a test uses so it never reaches the keychain.
    pub fn open_with_secrets(
        dir: &Path,
        defaults: ProviderDefaults,
        secrets: Box<dyn SecretStore>,
    ) -> Result<AppState> {
        crate::settings::open(dir, Some(defaults), secrets)
    }

    /// The same as [`AppState::open_with_secrets`], with the plugin directory stated too.
    pub fn open_at(
        dir: &Path,
        defaults: ProviderDefaults,
        secrets: Box<dyn SecretStore>,
        plugins: &Path,
    ) -> Result<AppState> {
        crate::settings::open_at(dir, Some(defaults), secrets, plugins)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the app's own state, read once and never rebuilt"
    )]
    pub(crate) fn build(
        dir: &Path,
        defaults: ProviderDefaults,
        stated: bool,
        servers: &[McpServerConfig],
        secrets: Box<dyn SecretStore>,
        plugins: &Path,
    ) -> Result<AppState> {
        let sources = Sources::open(plugins.to_path_buf(), servers)?;
        let live = Live::new(sources.registry().clone());
        let watch = watch::watch(&settings_file::path_in(dir), plugins);
        // A failure is the user's business, and the transcript is the only place the window ever
        // says things: it is a note in every session's column, never a window that will not open.
        let words = sources.words();
        let mut sessions = Vec::new();
        for meta in headers(dir)? {
            sessions.push(runtime(dir, meta, &live, &defaults, &words)?);
        }
        // The most recent session is the one the window opens on.
        let active = sessions.len().saturating_sub(1);
        Ok(AppState {
            dir: dir.to_path_buf(),
            sessions,
            active,
            live,
            sources,
            watch,
            defaults,
            stated,
            secrets,
        })
    }

    /// Opens a new session bound to `worktree` and makes it active.
    pub fn new_session(&mut self, worktree: &Path) -> Result<usize> {
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default();
        let title = worktree.file_name().map_or_else(
            || "session".to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let meta = SessionMeta {
            id: format!("{created}-{}", self.sessions.len()),
            title,
            worktree: worktree.to_path_buf(),
            branch: String::new(),
            model: self.defaults.model.clone(),
            created,
        };
        let words = self.sources.words();
        let runtime = runtime(&self.dir, meta, &self.live, &self.defaults, &words)?;
        self.sessions.push(runtime);
        self.active = self.sessions.len() - 1;
        Ok(self.active)
    }

    /// Shows the session at `index`. Switching is a state swap: nothing reloads from disk and no
    /// process restarts.
    pub fn switch(&mut self, index: usize) -> Result<()> {
        if index >= self.sessions.len() {
            return Err(Error::MissingSession(index));
        }
        self.active = index;
        Ok(())
    }

    /// The session the window shows.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Every session, in tab order.
    pub fn sessions(&self) -> &[SessionRuntime] {
        &self.sessions
    }

    /// The session the window shows, when there is one.
    pub fn active_session(&self) -> Option<&SessionRuntime> {
        self.sessions.get(self.active)
    }

    /// The session the window shows, for the composer and the answer keys.
    pub fn active_session_mut(&mut self) -> Option<&mut SessionRuntime> {
        self.sessions.get_mut(self.active)
    }

    /// The tools a session is offered right now, in the order the model sees them.
    pub fn tools(&self) -> Vec<ToolSpec> {
        self.live.specs()
    }

    /// Runs a tool by name through the live registry, in the active session's worktree.
    ///
    /// This is the session's dispatch path without a model: what a test or a script calls to prove
    /// a tool answers, and what makes a plugin's arrival visible without a turn.
    pub fn call_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> perch_tools::Result<ToolOutput> {
        let root = self
            .active_session()
            .map_or_else(|| self.dir.clone(), |session| session.meta.worktree.clone());
        self.live.call(name, args, &ToolCtx { root })
    }

    /// The MCP servers whose tools loaded, in configuration order.
    pub fn mcp_servers(&self) -> Vec<String> {
        self.sources.mcp_servers()
    }

    /// The provider a new session would be given: what a settings save changes.
    pub fn provider_defaults(&self) -> &ProviderDefaults {
        &self.defaults
    }

    /// What the settings panel opens with: the saved values and whether a key is stored.
    pub fn settings(&self) -> SettingsSnapshot {
        crate::settings::snapshot(&self.dir, &*self.secrets)
    }

    /// Writes the panel's draft and folds it into the provider defaults, with no restart.
    ///
    /// A session that is already running keeps the provider it was spawned with; the next session
    /// gets these values, which is what "applies immediately" can mean without restarting a turn.
    pub fn save_settings(
        &mut self,
        draft: SettingsDraft,
    ) -> std::result::Result<KeyStatus, String> {
        crate::settings::apply(&self.dir, &mut self.defaults, &*self.secrets, draft)
            .map_err(|error| error.to_string())
    }

    /// Rebuilds the tools from disk and swaps them in: the plugin directory, `settings.toml`, and
    /// the MCP servers it names.
    ///
    /// The rebuild starts from an empty registry, so what comes back is exactly what is on disk —
    /// a deleted plugin has nothing left to resurrect it. A turn already in flight is untouched:
    /// its calls read the registry when they start, so the next one sees the new list. Answers
    /// whether the settings file could be read; a file a person broke by hand leaves the tools
    /// alone rather than losing the ones that work.
    pub fn reload(&mut self) -> bool {
        let Ok(settings) = Settings::load(&self.dir) else {
            return false;
        };
        if !self.stated {
            self.follow(&settings);
        }
        self.sources.rebuild(&settings.mcp_servers);
        self.refresh_live();
        true
    }

    /// Sends the composer's text as a turn: the question is recorded, logged, and handed to the
    /// agent. A session whose worktree has gone refuses it and says why in its own transcript.
    pub fn prompt(&mut self, text: impl Into<String>) -> Result<()> {
        let text = text.into();
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return Err(Error::NoSession);
        };
        if !runtime.meta.worktree.exists() {
            let error = Error::MissingWorktree(runtime.meta.worktree.clone());
            say(&mut runtime.transcript, &format!("not sent: {error}"));
            return Err(error);
        }
        runtime.begin(&text);
        runtime.agent.prompt(text);
        Ok(())
    }

    /// Stops the running turn. It still ends, once, and the transcript says how.
    pub fn cancel(&mut self) {
        if let Some(runtime) = self.sessions.get(self.active) {
            runtime.agent.cancel();
        }
    }

    /// Stops the turn and settles whatever it is waiting on first: a question is dismissed and an
    /// approval is refused, so no call runs on the strength of an answer nobody gave.
    pub fn interrupt(&mut self) {
        match self
            .sessions
            .get(self.active)
            .and_then(SessionRuntime::pending)
            .cloned()
        {
            Some(Pending::Approval { .. }) => {
                self.answer(Decision::Deny);
            }
            Some(Pending::Question { .. }) => {
                self.answer(Answer::Cancelled);
            }
            None => {}
        }
        self.cancel();
    }

    /// Whether the active session is waiting on an approval.
    fn approval_waiting(&self) -> bool {
        matches!(
            self.sessions
                .get(self.active)
                .and_then(SessionRuntime::pending),
            Some(Pending::Approval { .. })
        )
    }

    /// Allows the call that is waiting, and reports whether one was.
    pub fn allow(&mut self) -> bool {
        self.approval_waiting() && self.answer(Decision::Allow)
    }

    /// Refuses it, and reports whether one was waiting.
    pub fn refuse(&mut self) -> bool {
        self.approval_waiting() && self.answer(Decision::Deny)
    }

    /// Answers the model's question with the option at `index`, counted from zero, and reports
    /// whether a question with that option was waiting.
    pub fn choose(&mut self, index: usize) -> bool {
        let Some(Pending::Question { options, .. }) = self
            .sessions
            .get(self.active)
            .and_then(SessionRuntime::pending)
        else {
            return false;
        };
        index < options.len() && self.answer(Answer::Picks(vec![index]))
    }

    /// Whether any session has a turn in flight. A session waiting on the user does not count:
    /// nothing can arrive until the answer does, so the window has nothing to poll for.
    pub fn busy(&self) -> bool {
        self.sessions
            .iter()
            .any(|session| session.is_running() && !session.is_waiting())
    }

    /// Every session's header, in tab order: what the window's session list is built from.
    pub fn metas(&self) -> Vec<SessionMeta> {
        self.sessions
            .iter()
            .map(|session| session.meta.clone())
            .collect()
    }

    /// Answers whatever the session is waiting on — an approval or a question — and reports
    /// whether anyone was waiting.
    pub fn answer(&mut self, reply: impl Into<crate::Reply>) -> bool {
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return false;
        };
        let sent = runtime.bridge.answer(reply.into());
        if sent {
            runtime.answered();
        }
        sent
    }

    /// Folds what the dispatch thread has posted into the active transcript, waiting at most
    /// `budget` in total and at most [`QUIET`] between two events.
    ///
    /// A session waiting for the user stops the loop: no further event can arrive until
    /// [`AppState::answer`] is called, so the frame is the frame's.
    ///
    /// The watcher is serviced first, and the plugins last: a change that arrived between frames
    /// is rebuilt before this frame draws, and the events the frame folded reach the plugins
    /// before it returns.
    pub fn pump(&mut self, budget: Duration) -> Pumped {
        if self.watch.as_ref().is_some_and(Watch::changed) {
            self.reload();
        }
        let mut pumped = self.pump_events(budget);
        if self.feed_plugins(pumped.changed || self.busy()) {
            pumped.changed = true;
        }
        pumped
    }

    /// Pumps until `done` holds, the session waits for the user, or `budget` runs out.
    ///
    /// The waiting case returns false rather than spinning: nothing can change until
    /// [`AppState::answer`] is called, so a caller that keeps pumping would only hold its frame.
    pub fn pump_until<F>(&mut self, budget: Duration, done: F) -> bool
    where
        F: Fn(&SessionRuntime) -> bool,
    {
        let deadline = Instant::now() + budget;
        loop {
            match self.sessions.get(self.active) {
                Some(runtime) if done(runtime) => return true,
                Some(runtime) if runtime.is_waiting() => return false,
                Some(_) => {}
                None => return false,
            }
            if Instant::now() >= deadline {
                return false;
            }
            self.pump(QUIET);
        }
    }

    /// The event loop without the watcher and the plugin feed.
    fn pump_events(&mut self, budget: Duration) -> Pumped {
        let deadline = Instant::now() + budget;
        let mut pumped = Pumped::default();
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return pumped;
        };
        loop {
            // Everything already posted is folded even with a budget of zero: the budget is how
            // long a pump may *wait* for the next event, never how much of the queue it may leave
            // behind for the next frame.
            let event = match runtime.bridge.events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Disconnected) => {
                    pumped.waiting = runtime.is_waiting();
                    return pumped;
                }
                Err(TryRecvError::Empty) => {
                    let now = Instant::now();
                    if now >= deadline {
                        pumped.waiting = runtime.is_waiting();
                        return pumped;
                    }
                    match runtime
                        .bridge
                        .events
                        .recv_timeout(QUIET.min(deadline - now))
                    {
                        Ok(event) => event,
                        // A quiet channel is a finished batch, and a disconnected one is a
                        // session whose thread has stopped. Neither is a reason to spin.
                        Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                            pumped.waiting = runtime.is_waiting();
                            return pumped;
                        }
                    }
                }
            };
            pumped.changed = true;
            if runtime.apply(event) {
                pumped.waiting = true;
                return pumped;
            }
        }
    }

    /// Hands the active session's new events to the plugins, and takes away the tools of a plugin
    /// that trapped or a server that died. Answers whether the live registry moved and needs
    /// swapping.
    fn feed_plugins(&mut self, active: bool) -> bool {
        let mut moved = false;
        if active && let Some(session) = self.sessions.get(self.active) {
            moved = self.sources.sync(&session.log);
        }
        moved |= self.sources.settle();
        if moved {
            self.refresh_live();
        }
        moved
    }

    /// Folds the settings file's provider into the defaults a new session would read.
    fn follow(&mut self, settings: &Settings) {
        self.defaults.base_url = settings.provider_base_url.clone();
        self.defaults.model = settings.model.clone();
        self.defaults.api_key = crate::settings::stored_key(&*self.secrets).unwrap_or_default();
    }

    /// Makes the rebuilt registry the one every dispatch reads from now on.
    fn refresh_live(&self) {
        self.live.store(self.sources.registry().clone());
    }
}

/// The plugin column, over the app's own sources: a window laid out over `AppState` alone shows
/// the same panels as the binary's window does.
impl Panels for AppState {
    fn panels(&self) -> Vec<PanelEntry> {
        self.sources.panels()
    }

    fn plugin_words(&self) -> Vec<String> {
        self.sources.words()
    }
}

/// Starts one session: its log, its agent, and the thread that will run its tools.
fn runtime(
    dir: &Path,
    meta: SessionMeta,
    live: &Live,
    defaults: &ProviderDefaults,
    notes: &[String],
) -> Result<SessionRuntime> {
    let log = SessionLog::open(dir, meta.clone())?;
    let model = if meta.model.is_empty() {
        defaults.model.clone()
    } else {
        meta.model.clone()
    };
    let (agent, events) = Agent::spawn(AgentConfig {
        base_url: defaults.base_url.clone(),
        api_key: SecretString::from(defaults.api_key.clone()),
        model,
        system: SYSTEM.to_string(),
        // The list the model is offered is fixed when the session starts; the registry a call
        // dispatches through is read at the call, so a reload lands without restarting a turn.
        tools: live
            .specs()
            .into_iter()
            .map(|spec| perch_agent::ToolSpec {
                name: spec.name,
                description: spec.description,
                parameters: spec.parameters,
            })
            .collect(),
    });
    let dispatcher = Arc::new(Mutex::new(Dispatcher::new()));
    let ctx = ToolCtx {
        root: meta.worktree.clone(),
    };
    let bridge = bridge::spawn(agent.clone(), events, live, ctx, &dispatcher);
    let mut transcript = Transcript::default();
    // A server that did not load is said once per session, in the column the user is reading.
    for note in notes {
        say(&mut transcript, note);
    }
    Ok(SessionRuntime::new(
        meta, transcript, log, agent, dispatcher, bridge,
    ))
}

/// Says `words` in a transcript as one item: the transcript has no note kind, so a note takes the
/// path a reply's text does. Only used while no turn streams — a startup note, a refused prompt —
/// so it never joins a reply.
fn say(transcript: &mut Transcript, words: &str) {
    transcript.apply(&AgentEvent::TextDelta(words.to_string()));
}

/// Every session header in `dir`, oldest first.
///
/// Only the first line is read: the header is written when the log is created, so a session whose
/// log is a gigabyte still costs one line. A file whose header no longer parses is skipped — its
/// worktree and model are unknown, and guessing them would open the wrong session.
fn headers(dir: &Path) -> Result<Vec<SessionMeta>> {
    let mut metas = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(metas),
        Err(error) => return Err(error.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut line = String::new();
        if BufReader::new(file).read_line(&mut line).is_err() {
            continue;
        }
        if let Ok(meta) = serde_json::from_str::<SessionMeta>(&line) {
            metas.push(meta);
        }
    }
    metas.sort_by_key(|meta| meta.created);
    Ok(metas)
}
