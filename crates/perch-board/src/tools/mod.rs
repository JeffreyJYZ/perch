//! The two tools the harness registers, and the pieces they agree on.

mod board;
mod wait;

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::cancel::{NeverStopped, Stopped};
use crate::error::{Error, Result};
use crate::session::{Fixed, SessionName};
use crate::store::{Read, Store};

pub use board::Board;
pub use wait::{BoardWait, WAIT_DEFAULT_MS, WAIT_MAX_MS};

/// The board and its blocking read, both posting as `session` for the project at `root`.
pub fn tools(root: &Path, session: &str) -> Vec<Arc<dyn perch_tools::Tool>> {
    tools_with_cancel(root, session, Arc::new(NeverStopped))
}

/// The same two tools, with the turn's cancellation to check while a wait is parked.
pub fn tools_with_cancel(
    root: &Path,
    session: &str,
    stopped: Arc<dyn Stopped>,
) -> Vec<Arc<dyn perch_tools::Tool>> {
    let fixed = Arc::new(Fixed(session.to_string()));
    tools_dynamic_with_cancel(root, fixed, stopped)
}

/// The same two tools, posting as whatever `session` names when a call runs.
///
/// One registry is built per process and shared by every session, so this is the shape a host
/// needs: the tools outlive the session they speak for, and the name is asked per call. The
/// cursor follows the name, so switching the slot switches which entries count as unseen.
pub fn tools_dynamic(
    root: &Path,
    session: Arc<dyn SessionName>,
) -> Vec<Arc<dyn perch_tools::Tool>> {
    tools_dynamic_with_cancel(root, session, Arc::new(NeverStopped))
}

/// [`tools_dynamic`] with the turn's cancellation to check while a wait is parked.
pub fn tools_dynamic_with_cancel(
    root: &Path,
    session: Arc<dyn SessionName>,
    stopped: Arc<dyn Stopped>,
) -> Vec<Arc<dyn perch_tools::Tool>> {
    let store = Arc::new(Store::with_source(root, root, session));
    vec![
        Arc::new(Board::new(Arc::clone(&store))),
        Arc::new(BoardWait::new(store, stopped)),
    ]
}

/// What a read looks like to the model. Nothing new is a sentence, not an empty answer, because
/// this is called at the start of a turn and a blank reply reads as a broken tool.
pub(crate) fn render(read: &Read) -> String {
    if read.entries.is_empty() {
        return "nothing new\n".to_string();
    }
    let mut out: String = read
        .entries
        .iter()
        .map(|entry| format!("{}\n", entry.render()))
        .collect();
    if read.remaining > 0 {
        out.push_str(&format!("[{} more]\n", read.remaining));
    }
    out
}

/// A string argument when it is one and is not blank; anything else of another shape is refused.
pub(crate) fn optional(args: &Value, key: &str) -> Result<Option<String>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.is_empty() => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(Error::Message(format!("`{key}` must be a string"))),
    }
}

/// The paths one post names. A non-string entry is refused rather than silently dropped, because
/// a post that loses its files reads as though it never had any.
pub(crate) fn files(args: &Value) -> Result<Vec<String>> {
    let Some(value) = args.get("files") else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err(Error::Message("`files` must be an array of paths".into()));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| Error::Message("`files` must be an array of paths".into()))
        })
        .collect()
}
