//! Opening the app: the sessions read from disk and the one agent each one runs.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::{Arc, Mutex};

use perch_agent::{Agent, AgentConfig, Dispatcher, SecretString, Tools};
use perch_store::secret::SecretStore;
use perch_store::settings::{self as settings_file, McpServerConfig};
use perch_store::{SessionLog, SessionMeta};
use perch_tools::ToolCtx;
use perch_ui::Transcript;

use super::{AppState, ProviderDefaults, Result, SYSTEM};
use crate::bridge;
use crate::live_registry::Live;
use crate::session::SessionRuntime;
use crate::sources::Sources;
use crate::watch;
use crate::worker::Worker;

impl AppState {
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
        let worker = Worker::start(sources);
        let snapshot = worker.latest();
        let live = Live::new(snapshot.registry.clone());
        let watch = watch::watch(&settings_file::path_in(dir), plugins);
        // A failure is the user's business, and the transcript is the only place the window ever
        // says things: it is a note in every session's column, never a window that will not open.
        let words = snapshot.words.clone();
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
            worker,
            snapshot,
            watch,
            defaults,
            stated,
            secrets,
        })
    }
}

/// Starts one session: its log, its agent, and the thread that will run its tools.
pub(super) fn runtime(
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
        // The list is read from the live registry at the start of every request, so a plugin that
        // arrives mid-session is offered on the next one — and a reload that lands mid-stream
        // leaves the request already out exactly as it was sent.
        tools: Tools::live(live_source(live)),
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
pub(super) fn say(transcript: &mut Transcript, words: &str) {
    transcript.apply(&perch_agent::AgentEvent::TextDelta(words.to_string()));
}

/// The app's live registry as the agent's tool source: a snapshot of `specs()` each time the agent
/// starts a request, so the next request carries whatever the last rebuild published.
///
/// The two `ToolSpec` types are separate because the wire shape is the agent crate's; the map here
/// is the only conversion, and it is what keeps the model's list and the dispatch list the same
/// list.
fn live_source(live: &Live) -> impl Fn() -> Vec<perch_agent::ToolSpec> + Send + Sync + 'static {
    let live = live.clone();
    move || {
        live.specs()
            .into_iter()
            .map(|spec| perch_agent::ToolSpec {
                name: spec.name,
                description: spec.description,
                parameters: spec.parameters,
            })
            .collect()
    }
}

/// Every session header in `dir`, oldest first.
///
/// Only the first line is read: the header is written when the log is created, so a session whose
/// log is a gigabyte still costs one line. A file whose header no longer parses is skipped — its
/// worktree and model are unknown, and guessing them would open the wrong session.
pub(super) fn headers(dir: &Path) -> Result<Vec<SessionMeta>> {
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
