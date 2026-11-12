//! The model's plan: one todo list per worktree root, replaced whole by `todowrite`.
//!
//! The table is process-wide and keyed by the worktree root for the same reason [`super::job`]'s
//! is: a registry is rebuilt between turns, so a list held by a tool instance would vanish while
//! the turn that wrote it is still running. Keying by root also keeps two sessions in one process
//! from reading each other's plan.
//!
//! A write replaces the list rather than editing it, so a call needs no ids to address items and
//! calling it twice with the same list changes nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// How many todos one list may hold.
pub const MAX_TODOS: usize = 32;

/// Where one todo is: not started, the one being worked on, or finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    InProgress,
    Completed,
}

impl Status {
    /// The status a written word names, or why it names none.
    pub fn parse(word: &str) -> Result<Status> {
        match word {
            "pending" => Ok(Status::Pending),
            "in_progress" => Ok(Status::InProgress),
            "completed" => Ok(Status::Completed),
            other => Err(Error::Message(format!(
                "unknown todo status `{other}`; use `pending`, `in_progress` or `completed`"
            ))),
        }
    }
}

/// One item of the plan, in the JSON shape the model writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    pub content: String,
    pub status: Status,
}

fn lists() -> &'static Mutex<HashMap<PathBuf, Vec<Todo>>> {
    static LISTS: OnceLock<Mutex<HashMap<PathBuf, Vec<Todo>>>> = OnceLock::new();
    LISTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A poisoned lock is not a reason to lose every list, so the guard is recovered instead.
fn lock() -> MutexGuard<'static, HashMap<PathBuf, Vec<Todo>>> {
    lists()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn key(root: &Path) -> PathBuf {
    root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
}

/// This worktree's list, empty when nothing was written.
pub fn todos(root: &Path) -> Vec<Todo> {
    lock().get(&key(root)).cloned().unwrap_or_default()
}

/// Replaces this worktree's list with `next`, refusing a list that breaks the rules.
///
/// The rules are checked before anything is stored, so a refused call leaves the list exactly as
/// it was: the model's next `todoread` sees the plan it last wrote.
pub(super) fn replace(root: &Path, next: Vec<Todo>) -> Result<()> {
    check(&next)?;
    lock().insert(key(root), next);
    Ok(())
}

fn check(todos: &[Todo]) -> Result<()> {
    if todos.len() > MAX_TODOS {
        return Err(Error::Message(format!(
            "at most {MAX_TODOS} todos fit in one list; this one had {}",
            todos.len()
        )));
    }
    let working = todos
        .iter()
        .filter(|todo| todo.status == Status::InProgress)
        .count();
    if working > 1 {
        return Err(Error::Message(format!(
            "at most one todo may be `in_progress` at a time; this list marks {working}"
        )));
    }
    Ok(())
}
