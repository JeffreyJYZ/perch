//! The app's session state: sessions read from their logs, one live agent each, and the
//! transcript, tools and dispatch thread those sessions answer with.
//!
//! [`AppState`] is the window's whole model of a live conversation. It owns the turn's events
//! only through [`AppState::pump`], which folds whatever the dispatch thread posted into the
//! active transcript — so the thread that draws is the thread that mutates, and a turn's arrival
//! can never race a frame. Everything a turn needs from a person blocks that dispatch thread and
//! nothing else; the answer comes back through [`AppState::answer`].

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use perch_agent::{
    Agent, AgentConfig, AgentEvent, AgentHandle, Dispatcher, EndReason, SecretString,
};
use perch_mcp::McpHost;
use perch_store::{SessionEvent, SessionLog, SessionMeta, Settings, settings::McpServerConfig};
use perch_tools::{ToolCtx, ToolRegistry};
use perch_ui::Transcript;

use crate::bridge::{self, Bridge, BridgeEvent, Reply};

/// How long one pump waits for the next event before calling the session quiet.
///
/// Short enough that a frame is never held, long enough that a stream's deltas arrive in batches.
pub const QUIET: Duration = Duration::from_millis(25);

/// What the transcript says when a turn is cancelled.
///
/// A word, like every other state the window shows: the turn stopped, and saying so is the only
/// way the column is not silently missing an answer.
pub const CANCELLED: &str = "cancelled";

/// The system message every session starts from.
const SYSTEM: &str = "You are perch, a coding agent working in the session's worktree. Use the \
                      tools you are given; every path is relative to that worktree.";

/// The keychain account the provider key is stored under.
const KEY_ACCOUNT: &str = "provider";

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

/// What a session is waiting on, and therefore what [`AppState::answer`] settles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// A call that needs the user's decision before it runs.
    Approval { id: String, name: String },
    /// The model's question.
    Question {
        prompt: String,
        options: Vec<String>,
        multiple: bool,
    },
}

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

/// One session: its log, its agent, its transcript, and the thread that runs its tools.
pub struct SessionRuntime {
    pub meta: SessionMeta,
    pub transcript: Transcript,
    pub log: SessionLog,
    pub agent: AgentHandle,
    /// What remembers an "allow for session". One per session, because allowing `bash` here
    /// must not silently allow it in another session.
    ///
    /// Held by the session, not only by the dispatch thread, so the same allowances are reachable
    /// for a later reset without a second dispatcher appearing.
    #[allow(
        dead_code,
        reason = "held for the session's lifetime, read through the dispatch thread"
    )]
    dispatcher: Arc<Mutex<Dispatcher>>,
    bridge: Bridge,
    pending: Option<Pending>,
    turns: u32,
    /// The text this turn has streamed, so the log keeps the reply the transcript showed.
    open_text: String,
}

impl SessionRuntime {
    /// What this session is waiting for, when it is waiting.
    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    /// Whether the session is waiting for the user before it can go on.
    pub fn is_waiting(&self) -> bool {
        self.pending.is_some()
    }

    /// How many turns have ended. One is the promise; a count makes a second one visible.
    pub fn turns_ended(&self) -> u32 {
        self.turns
    }

    /// This session's transcript, which the shell draws.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// Folds one posted event into the transcript and the log, and reports whether the session
    /// is now waiting on the user.
    fn apply(&mut self, event: BridgeEvent) -> bool {
        match event {
            BridgeEvent::Text(text) => {
                self.transcript.apply(&AgentEvent::TextDelta(text.clone()));
                self.open_text.push_str(&text);
            }
            BridgeEvent::ToolCall(call) => {
                self.transcript.apply(&AgentEvent::ToolCall(call.clone()));
                // The log is the conversation's only durable copy; a write failing is worth
                // dropping rather than stopping the turn the user is watching.
                let _ = self.log.append(&SessionEvent::ToolCall {
                    id: call.id,
                    name: call.name,
                    args: call.args,
                });
            }
            BridgeEvent::ToolResult { id, ok, text } => {
                self.transcript.apply(&AgentEvent::ToolResult {
                    id: id.clone(),
                    ok,
                    text: text.clone(),
                });
                let _ = self.log.append(&SessionEvent::ToolResult { id, ok, text });
            }
            BridgeEvent::Approval { id, name, preview } => {
                // A tool that asks and shows nothing keeps its card saying "running": the column
                // has one approval state and it is the preview's. The wait below is what holds
                // the turn either way.
                if let Some(preview) = preview {
                    self.transcript.request_approval(&id, preview);
                }
                self.pending = Some(Pending::Approval { id, name });
                return true;
            }
            BridgeEvent::Question {
                prompt,
                options,
                multiple,
            } => {
                self.transcript
                    .apply(&AgentEvent::TextDelta(question_text(&prompt, &options)));
                self.pending = Some(Pending::Question {
                    prompt,
                    options,
                    multiple,
                });
                return true;
            }
            BridgeEvent::TurnEnd { reason, usage } => {
                self.transcript.apply(&AgentEvent::TurnEnd {
                    reason: reason.clone(),
                    usage,
                });
                if reason == EndReason::Cancelled {
                    // The column has no note item and this lane does not own its enum, so the
                    // word takes the same path a reply's text does — one item, saying the turn
                    // stopped. When the transcript gains a note item this is one line to change.
                    self.transcript
                        .apply(&AgentEvent::TextDelta(CANCELLED.into()));
                }
                let reply = std::mem::take(&mut self.open_text);
                if !reply.is_empty() {
                    let _ = self.log.append(&SessionEvent::Assistant { text: reply });
                }
                let _ = self.log.append(&SessionEvent::TurnEnd {
                    reason: reason_word(&reason),
                    tokens_in: usage.input,
                    tokens_out: usage.output,
                });
                self.pending = None;
                self.turns += 1;
            }
        }
        false
    }
}

/// Every open session, which one the window shows, and the tools they share.
pub struct AppState {
    dir: PathBuf,
    sessions: Vec<SessionRuntime>,
    active: usize,
    /// The tools every session's model may call: the six built-ins, `ask_user`, and whatever MCP
    /// servers contributed at start.
    registry: Arc<ToolRegistry>,
    /// The MCP servers that loaded, kept so their notes can be reported and their tools dropped.
    mcp: McpHost,
    defaults: ProviderDefaults,
}

impl AppState {
    /// Opens every session whose log is in `dir`, reading the provider from settings and the key
    /// from the keychain.
    ///
    /// A keychain that refuses is not fatal: the window opens and a turn reports the provider's
    /// rejection, which is a better failure than a window that will not start.
    pub fn open(dir: &Path) -> Result<AppState> {
        let settings = Settings::load(dir)?;
        let api_key = perch_store::secret::load(KEY_ACCOUNT)
            .ok()
            .flatten()
            .unwrap_or_default();
        let defaults = ProviderDefaults {
            base_url: settings.provider_base_url.clone(),
            api_key,
            model: settings.model.clone(),
        };
        Self::build(dir, defaults, &settings.mcp_servers)
    }

    /// The same, with the provider stated instead of read: what a test or a scripted run uses.
    ///
    /// MCP servers still come from settings, so a session started this way sees exactly the tools
    /// the window would give it.
    pub fn open_with(dir: &Path, defaults: ProviderDefaults) -> Result<AppState> {
        let settings = Settings::load(dir)?;
        Self::build(dir, defaults, &settings.mcp_servers)
    }

    fn build(
        dir: &Path,
        defaults: ProviderDefaults,
        servers: &[McpServerConfig],
    ) -> Result<AppState> {
        let mut registry = Dispatcher::builtin_registry();
        // A server that fails is reported by the host and skipped, so one broken server cannot
        // stop the app from opening.
        let mcp = McpHost::connect_all(servers, &mut registry)?;
        let registry = Arc::new(registry);
        let mut sessions = Vec::new();
        for meta in headers(dir)? {
            sessions.push(runtime(dir, meta, &registry, &defaults)?);
        }
        // The most recent session is the one the window opens on.
        let active = sessions.len().saturating_sub(1);
        Ok(AppState {
            dir: dir.to_path_buf(),
            sessions,
            active,
            registry,
            mcp,
            defaults,
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
        let runtime = runtime(&self.dir, meta, &self.registry, &self.defaults)?;
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

    /// The MCP servers whose tools loaded, in configuration order.
    pub fn mcp_servers(&self) -> Vec<String> {
        self.mcp.server_names()
    }

    /// Sends the composer's text as a turn: the question is recorded, logged, and handed to the
    /// agent.
    pub fn prompt(&mut self, text: impl Into<String>) -> Result<()> {
        let text = text.into();
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return Err(Error::NoSession);
        };
        if !runtime.meta.worktree.exists() {
            return Err(Error::MissingWorktree(runtime.meta.worktree.clone()));
        }
        runtime.open_text.clear();
        runtime.transcript.push_user(text.clone());
        let _ = runtime
            .log
            .append(&SessionEvent::User { text: text.clone() });
        runtime.agent.prompt(text);
        Ok(())
    }

    /// Stops the running turn. It still ends, once, and the transcript says how.
    pub fn cancel(&mut self) {
        if let Some(runtime) = self.sessions.get(self.active) {
            runtime.agent.cancel();
        }
    }

    /// Answers whatever the session is waiting on — an approval or a question — and reports
    /// whether anyone was waiting.
    pub fn answer(&mut self, reply: impl Into<Reply>) -> bool {
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return false;
        };
        let sent = runtime.bridge.answer(reply.into());
        if sent {
            runtime.pending = None;
        }
        sent
    }

    /// Folds what the dispatch thread has posted into the active transcript, waiting at most
    /// `budget` in total and at most [`QUIET`] between two events.
    ///
    /// Returns whether the session is now waiting for the user, which is where a caller stops: no
    /// further event can arrive until it is answered.
    pub fn pump(&mut self, budget: Duration) -> bool {
        let deadline = Instant::now() + budget;
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return false;
        };
        loop {
            let now = Instant::now();
            if now >= deadline {
                return runtime.is_waiting();
            }
            let wait = QUIET.min(deadline - now);
            match runtime.bridge.events.recv_timeout(wait) {
                Ok(event) => {
                    if runtime.apply(event) {
                        return true;
                    }
                }
                // A quiet channel is a finished batch, and a disconnected one is a session whose
                // thread has stopped. Neither is a reason to spin.
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    return runtime.is_waiting();
                }
            }
        }
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
}

/// Starts one session: its log, its agent, and the thread that will run its tools.
fn runtime(
    dir: &Path,
    meta: SessionMeta,
    registry: &Arc<ToolRegistry>,
    defaults: &ProviderDefaults,
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
        // One list for every session, so the model is offered exactly what the registry holds.
        tools: registry
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
    let bridge = bridge::spawn(events, Arc::clone(registry), ctx, Arc::clone(&dispatcher));
    Ok(SessionRuntime {
        meta,
        transcript: Transcript::default(),
        log,
        agent,
        dispatcher,
        bridge,
        pending: None,
        turns: 0,
        open_text: String::new(),
    })
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

/// The question and its options as the transcript shows them.
fn question_text(prompt: &str, options: &[String]) -> String {
    let mut text = prompt.to_string();
    for (index, option) in options.iter().enumerate() {
        text.push_str(&format!("\n{}. {option}", index + 1));
    }
    text
}

/// How a turn's ending is written in the log.
fn reason_word(reason: &EndReason) -> String {
    match reason {
        EndReason::Done => "done".to_string(),
        EndReason::Cancelled => CANCELLED.to_string(),
        EndReason::Errored(why) => format!("errored: {why}"),
    }
}
