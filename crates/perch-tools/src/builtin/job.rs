//! Background jobs: one log file per job and a table per worktree root.
//!
//! The table is process-wide and keyed by the worktree root rather than held by [`super::Bash`]:
//! a registry is rebuilt between turns, so a job that lived only in that instance would vanish
//! mid-flight. Keying by root also keeps two sessions in one process from seeing each other.

mod kill;
mod log;
mod spawn;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use crate::{Error, Result};

pub(super) use kill::kill;
pub(super) use log::read;
pub(super) use spawn::start;

/// How many jobs may run at once in one worktree.
pub const JOB_MAX_CONCURRENT: usize = 16;

/// What a job is doing, or how it stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Running,
    Exited(Option<i32>),
    Killed(Option<i32>),
}

impl State {
    pub(super) fn label(self) -> &'static str {
        match self {
            State::Running => "running",
            State::Exited(_) => "exited",
            State::Killed(_) => "killed",
        }
    }

    pub(super) fn code(self) -> Option<i32> {
        match self {
            State::Running => None,
            State::Exited(code) | State::Killed(code) => code,
        }
    }

    /// One phrase naming the state and, when there is one, its exit code.
    pub(super) fn describe(self) -> String {
        match (self, self.code()) {
            (State::Running, _) => "running".to_string(),
            (State::Exited(_), Some(code)) => format!("exited {code}"),
            (State::Exited(_), None) => "exited on a signal".to_string(),
            (State::Killed(_), Some(code)) => format!("killed (exit code {code})"),
            (State::Killed(_), None) => "killed".to_string(),
        }
    }
}

/// One background child and everything needed to report on it later.
pub(super) struct Job {
    pub(super) id: String,
    pub(super) command: String,
    pub(super) pid: u32,
    pub(super) log: PathBuf,
    pub(super) started: Instant,
    /// Frozen when the job stops, so a finished job's elapsed time never keeps growing.
    pub(super) elapsed: Option<Duration>,
    pub(super) state: State,
    pub(super) child: Child,
    /// Held open on purpose: the app dying closes it, which is what kills the group.
    pub(super) _stdin: Option<ChildStdin>,
}

impl Job {
    pub(super) fn elapsed(&self) -> Duration {
        self.elapsed.unwrap_or_else(|| self.started.elapsed())
    }
}

/// What `jobs list` prints for one job.
pub(super) struct View {
    pub(super) id: String,
    pub(super) command: String,
    pub(super) state: &'static str,
    pub(super) code: Option<i32>,
    pub(super) elapsed: Duration,
    pub(super) pid: u32,
    pub(super) log: PathBuf,
}

#[derive(Default)]
pub(super) struct Session {
    next: usize,
    jobs: Vec<Arc<Mutex<Job>>>,
}

fn sessions() -> &'static Mutex<HashMap<PathBuf, Arc<Mutex<Session>>>> {
    static SESSIONS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<Session>>>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A poisoned lock is not a reason to lose every job, so the guard is recovered instead.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(super) fn session(root: &Path) -> Result<Arc<Mutex<Session>>> {
    let key = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut map = lock(sessions());
    let slot = map.entry(key).or_insert_with(|| {
        Arc::new(Mutex::new(Session {
            next: 1,
            jobs: Vec::new(),
        }))
    });
    Ok(Arc::clone(slot))
}

/// Reaps a job that finished since the last look, so it is never reported as running.
pub(super) fn refresh(job: &Arc<Mutex<Job>>) {
    let mut job = lock(job);
    if job.state != State::Running {
        return;
    }
    if let Ok(Some(status)) = job.child.try_wait() {
        job.state = State::Exited(status.code());
        job.elapsed = Some(job.started.elapsed());
    }
}

/// Every job in this worktree, oldest first.
pub(super) fn list(root: &Path) -> Result<Vec<View>> {
    let session = session(root)?;
    let guard = lock(&session);
    let mut views = Vec::with_capacity(guard.jobs.len());
    for entry in &guard.jobs {
        refresh(entry);
        let job = lock(entry);
        views.push(View {
            id: job.id.clone(),
            command: job.command.clone(),
            state: job.state.label(),
            code: job.state.code(),
            elapsed: job.elapsed(),
            pid: job.pid,
            log: job.log.clone(),
        });
    }
    Ok(views)
}

pub(super) fn find(root: &Path, id: &str) -> Result<Arc<Mutex<Job>>> {
    let session = session(root)?;
    let guard = lock(&session);
    guard
        .jobs
        .iter()
        .find(|job| lock(job).id == id)
        .cloned()
        .ok_or_else(|| {
            Error::Message(format!(
                "no job named `{id}`; `jobs` with `op: \"list\"` shows the ones this session started"
            ))
        })
}
