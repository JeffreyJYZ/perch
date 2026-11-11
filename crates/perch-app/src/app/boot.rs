//! Opening the app: the sessions read from disk and the one agent each one runs.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::{Arc, Mutex};

use perch_agent::{Agent, AgentConfig, Dispatcher, Endpoint, Model, Tools};
use perch_store::secret::SecretStore;
use perch_store::settings::{self as settings_file, McpServerConfig};
use perch_store::{SessionLog, SessionMeta};
use perch_tools::ToolCtx;

use super::tools::live_source;
pub(super) use super::words::say;
use super::{AppState, ProviderDefaults, Result};

mod instructions;
mod replay;
mod rules;
pub use replay::MAX_EXCHANGES;

use crate::bridge;
use crate::live_registry::Live;
use crate::session::SessionRuntime;
use crate::update::{Plan, Worker as UpdateWorker};
use crate::watch;
use crate::worker::Worker;

use super::subagent;

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
        update: Option<Plan>,
    ) -> Result<AppState> {
        // The tools are opened on the worker's own thread: the plugin engine and the MCP
        // handshakes are what used to stand between the launch and the first frame. What is read
        // here is a snapshot of the built-ins; the window draws with it and the worker replaces it.
        // The provider a child `task` turn asks is fixed here too, because the sources are built
        // before any session exists and a child always needs a model of its own.
        let agents = subagent::Agents::new(&defaults);
        let worker = Worker::start(plugins.to_path_buf(), servers.to_vec(), Some(agents));
        let snapshot = worker.latest();
        let live = Live::new(snapshot.registry.clone());
        let watch = watch::watch(&settings_file::path_in(dir), plugins);
        // The check is posted to its own thread here and not waited for: a manifest that takes
        // thirty seconds to answer must not be one the window spends not drawing. The URL and
        // transport are kept so a save that changes the manifest restarts the thread instead of
        // waiting for the app to be reopened.
        let update_url = update
            .as_ref()
            .map(|plan| plan.url.clone())
            .unwrap_or_default();
        let update_transport = update.as_ref().map(|plan| Arc::clone(&plan.transport));
        let update =
            update.map(|plan| UpdateWorker::start(plan.url, dir.to_path_buf(), plan.transport));
        let update_status = update.as_ref().map(UpdateWorker::latest);
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
            spoken: Vec::new(),
            watch,
            update,
            update_url,
            update_transport,
            update_status,
            listings: Arc::new(perch_agent::HttpTransport),
            limits: std::collections::HashMap::new(),
            models_job: None,
            defaults,
            stated,
            secrets,
            titles: super::title::Titles::default(),
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
    // What this session did before it was closed: the rows the column draws, and the record the
    // model's system message carries so the first turn of this run is not the first of its life.
    // Read here and never sent, so opening a session talks to nobody.
    let replayed = replay::read(&log)?;
    // The project's own instructions travel in the system message, read from this session's
    // worktree: a session opened on another repository must be told that repository's rules.
    let (system, large) = instructions::system(&meta.worktree);
    let system = replayed.seeded(system);
    // The saved rules are read per session, which is one small file and the only place a session
    // is built. A file that will not parse leaves no rules at all, so every gated call asks.
    let (rules, refused) = rules::load(dir);
    let model = if meta.model.is_empty() {
        defaults.model.clone()
    } else {
        meta.model.clone()
    };
    // The handle is kept by the session: a model picked in the composer is the next request's,
    // because the agent reads this handle when it starts one.
    let model = Model::new(model);
    // The address and key are a handle too: a settings save reaches the session already on
    // screen, so a key added after the window opened is not answered `401` forever.
    let endpoint = Endpoint::new(defaults.base_url.clone(), defaults.api_key.clone());
    let (agent, events) = Agent::spawn(AgentConfig {
        endpoint: endpoint.clone(),
        model: model.clone(),
        system,
        // The list is read from the live registry at the start of every request, so a plugin that
        // arrives mid-session is offered on the next one — and a reload that lands mid-stream
        // leaves the request already out exactly as it was sent.
        tools: Tools::live(live_source(live)),
    });
    // The `task` tool is built before this session exists, so the child it starts finds its
    // parent's cancellation here: by worktree, which is the only thing a call carries. Without
    // this line a cancel would leave a child working for nobody.
    subagent::watching(&meta.worktree, agent.watch());
    let dispatcher = Arc::new(Mutex::new(Dispatcher::new().with_rules(rules.clone())));
    let ctx = ToolCtx {
        root: meta.worktree.clone(),
    };
    let bridge = bridge::spawn(agent.clone(), events, live, ctx, &dispatcher);
    let mut transcript = replayed.transcript;
    // A server that did not load is said once per session, in the column the user is reading.
    for note in notes {
        say(&mut transcript, note);
    }
    if let Some(large) = large {
        say(&mut transcript, &large);
    }
    // A saved permission that could not be read is said where a settings error is said, in the
    // column of the session it would have applied to.
    if let Some(refused) = refused {
        say(&mut transcript, &refused);
    }
    Ok(SessionRuntime::new(
        meta, transcript, log, agent, dispatcher, bridge, model, endpoint,
    ))
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
