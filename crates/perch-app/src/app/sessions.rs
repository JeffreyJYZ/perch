//! Switching between sessions, and reading what one offers.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use perch_store::SessionMeta;
use perch_tools::{ToolCtx, ToolOutput, ToolSpec};

use super::boot::runtime;
use super::{AppState, Error, Result};
use crate::session::SessionRuntime;

impl AppState {
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
            branch: branch_words(worktree),
            model: self.defaults.model.clone(),
            created,
        };
        let words = self.snapshot.words.clone();
        let runtime = runtime(&self.dir, meta, &self.live, &self.defaults, &words)?;
        self.sessions.push(runtime);
        self.active = self.sessions.len() - 1;
        Ok(self.active)
    }

    /// Opens a session on a folder a person picked in the window.
    ///
    /// The folder is taken as chosen: a home directory or a root is a decision, not a mistake,
    /// so it is canonicalised and opened like any other. Only the launch directory is guarded.
    pub fn open_folder(&mut self, path: &Path) -> Result<usize> {
        let root = crate::launch::directory(path.to_path_buf()).map_err(Error::Folder)?;
        self.new_session(&root)
    }

    /// Shows the session at `index`. Switching is a state swap: nothing reloads from disk and no
    /// process restarts. The branch is re-read on the way, so a checkout made in the terminal is
    /// on the row the moment the session is shown.
    pub fn switch(&mut self, index: usize) -> Result<()> {
        if index >= self.sessions.len() {
            return Err(Error::MissingSession(index));
        }
        self.active = index;
        self.refresh_branch();
        Ok(())
    }

    /// Re-reads the active session's branch and rewrites its header when it moved.
    ///
    /// A detached head is `HEAD`, a directory that is not a repository is empty, and neither is
    /// an error: the row says what the worktree says, and a git call that fails changes nothing.
    pub fn refresh_branch(&mut self) {
        let Some(runtime) = self.sessions.get_mut(self.active) else {
            return;
        };
        let words = branch_words(&runtime.meta.worktree);
        if runtime.meta.branch == words {
            return;
        }
        runtime.meta.branch = words.clone();
        let _ = runtime.log.set_branch(&words);
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
        self.snapshot.servers.clone()
    }

    /// Every session's header, in tab order: what the window's session list is built from.
    pub fn metas(&self) -> Vec<SessionMeta> {
        self.sessions
            .iter()
            .map(|session| session.meta.clone())
            .collect()
    }
}

/// The branch word a row carries: its name, `HEAD` when detached, and `↑n`/`↓n` when the branch
/// has drifted from its upstream. A directory that is not a repository is an empty word.
pub(crate) fn branch_words(root: &Path) -> String {
    let Some(branch) = perch_files::branch_of(root) else {
        return String::new();
    };
    let mut words = if branch.detached || branch.name.is_empty() {
        "HEAD".to_string()
    } else {
        branch.name
    };
    if branch.ahead > 0 {
        words.push_str(&format!(" ↑{}", branch.ahead));
    }
    if branch.behind > 0 {
        words.push_str(&format!(" ↓{}", branch.behind));
    }
    words
}
