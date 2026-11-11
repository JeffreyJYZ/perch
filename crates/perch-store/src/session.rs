//! The append-only session log: one JSONL file per session.
//!
//! The first line is the [`SessionMeta`] header the session list reads at start; every later
//! line is one [`SessionEvent`]. Appends write a whole line at a time, so a crash costs at most
//! the line being written, and a torn last line is dropped when the file is read back.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

mod fork;
mod logged;
pub use logged::Logged;

/// Everything the session list needs, written as the first line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub worktree: PathBuf,
    pub branch: String,
    pub model: String,
    pub created: u64,
}

/// One thing that happened in a session, tagged by `kind` in the JSONL file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEvent {
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        args: serde_json::Value,
    },
    ToolResult {
        id: String,
        ok: bool,
        text: String,
    },
    TurnEnd {
        reason: String,
        tokens_in: u32,
        tokens_out: u32,
    },
    /// A line the log keeps that is not part of the conversation: a title it could not name, a
    /// session removed, anything a person never sees in the transcript.
    Note {
        text: String,
    },
}

/// The session file for `id`, `<dir>/<id>.jsonl`.
pub fn meta_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.jsonl"))
}

/// An open session file.
#[derive(Debug)]
pub struct SessionLog {
    path: PathBuf,
    file: File,
    meta: SessionMeta,
}

impl SessionLog {
    /// Open the log for `meta`, creating the directory, the file and its header when absent.
    pub fn open(dir: &Path, meta: SessionMeta) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = meta_path(dir, &meta.id);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let header = match file.metadata()?.len() {
            0 => Some(serde_json::to_string(&meta)?),
            _ => None,
        };
        let mut log = Self { path, file, meta };
        if let Some(header) = header {
            log.write_line(&header)?;
        }
        Ok(log)
    }

    /// Append one event as a whole line, flushed so a reader sees it immediately.
    pub fn append(&mut self, event: &SessionEvent) -> Result<()> {
        let line = serde_json::to_string(event)?;
        self.write_line(&line)
    }

    /// Every readable event, in file order. A line that does not parse is dropped.
    ///
    /// [`SessionLog::logged`] is the same read with the number of dropped lines, which is what a
    /// reader that has to tell the truth about a damaged log wants.
    pub fn events(&self) -> Result<Vec<SessionEvent>> {
        Ok(self.logged()?.events)
    }

    /// The header this log was opened with.
    pub fn meta(&self) -> &SessionMeta {
        &self.meta
    }

    /// Rewrites the header so a session whose model changed remembers the new one.
    ///
    /// Only the first line is rewritten; every event after it is kept byte for byte, so a log a
    /// crash left mid-line is not repaired or reordered by this.
    pub fn set_model(&mut self, model: &str) -> Result<()> {
        if self.meta.model == model {
            return Ok(());
        }
        self.meta.model = model.to_string();
        self.rewrite_header()
    }

    /// Rewrites the header so the row carries the branch the worktree is on now.
    pub fn set_branch(&mut self, branch: &str) -> Result<()> {
        if self.meta.branch == branch {
            return Ok(());
        }
        self.meta.branch = branch.to_string();
        self.rewrite_header()
    }

    /// Rewrites the header so the session carries the title a person gave it.
    ///
    /// The title is the first line's, like the model, so a session read at start is listed under
    /// the name it was renamed to; every event after the header is kept byte for byte.
    pub fn set_title(&mut self, title: &str) -> Result<()> {
        let title = title.trim();
        if self.meta.title == title {
            return Ok(());
        }
        self.meta.title = title.to_string();
        self.rewrite_header()
    }

    /// Deletes this session's log, closing the file first. The last copy of the conversation goes
    /// with it: a delete that left the file behind would put the session back at the next start.
    pub fn remove(self) -> Result<()> {
        let path = self.path.clone();
        drop(self);
        std::fs::remove_file(path)?;
        Ok(())
    }

    /// The file this log appends to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn rewrite_header(&mut self) -> Result<()> {
        let text = std::fs::read_to_string(&self.path)?;
        let rest = text.split_once('\n').map_or("", |(_, rest)| rest);
        let header = serde_json::to_string(&self.meta)?;
        let scratch = self.path.with_extension("jsonl.tmp");
        {
            let mut file = File::create(&scratch)?;
            file.write_all(header.as_bytes())?;
            file.write_all(b"\n")?;
            file.write_all(rest.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&scratch, &self.path)?;
        self.file = OpenOptions::new().append(true).open(&self.path)?;
        Ok(())
    }

    fn write_line(&mut self, line: &str) -> Result<()> {
        writeln!(self.file, "{line}")?;
        self.file.flush()?;
        Ok(())
    }
}
