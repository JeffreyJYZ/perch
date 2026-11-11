//! Forking a session: a second log holding the same conversation, up to a turn boundary.
//!
//! A fork is a copy taken out of the source's file, never a reference to it and never a rewrite of
//! it: the source keeps appending to its own log and its bytes are the same after the fork as
//! before it. The header the copy carries is written for the new id, so opening the fork does not
//! think it is the session it came from.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{SessionEvent, SessionLog, SessionMeta, meta_path};
use crate::{Error, Result};

impl SessionLog {
    /// Writes a new session beside `from`'s log, holding the conversation up to a boundary.
    ///
    /// `at` is how many finished turns to keep — `Some(1)` is the first exchange, `None` is the
    /// whole log. A fork is only ever cut after a `TurnEnd`: a copy taken inside a turn would hold
    /// a tool call whose result never arrived, and the replayed column would draw a card that never
    /// finished. A boundary that would land inside a turn is refused rather than rounded either way.
    pub fn fork(dir: &Path, from: &SessionMeta, at: Option<usize>) -> Result<SessionMeta> {
        let logged = super::logged::read(&meta_path(dir, &from.id))?;
        let events = boundary(&logged.events, at)?;
        let meta = fork_meta(dir, from);
        let mut log = SessionLog::open(dir, meta.clone())?;
        for event in events {
            log.append(event)?;
        }
        Ok(meta)
    }
}

/// The events `events` keeps for a fork taken after `at` finished turns.
///
/// A turn starts at a `User` and ends at its `TurnEnd`; everything before the first `User` is the
/// log's own bookkeeping and always comes along. `None` keeps the whole log, including a tail whose
/// turn never ended, because that is the session as it stands rather than a cut through it.
fn boundary(events: &[SessionEvent], at: Option<usize>) -> Result<&[SessionEvent]> {
    let Some(at) = at else {
        return Ok(events);
    };
    if at == 0 {
        return Err(Error::Fork(
            "a fork keeps at least one finished turn, so the boundary cannot be before the first"
                .to_string(),
        ));
    }
    let mut turns = 0;
    for (index, event) in events.iter().enumerate() {
        match event {
            SessionEvent::User { .. } => turns += 1,
            SessionEvent::TurnEnd { .. } if turns == at => return Ok(&events[..=index]),
            _ => {}
        }
    }
    if turns < at {
        return Err(Error::Fork(format!(
            "the log holds {turns} finished turn(s), so there is no turn {at} to fork after"
        )));
    }
    Err(Error::Fork(format!(
        "turn {at} never reached a `TurnEnd`, so a fork after it would land inside the turn: its \
         tool cards would be copied with no result"
    )))
}

/// The header the fork carries: an id of its own, a title saying where it came from, and the
/// source's worktree, branch and model. Its creation time is later than the source's, so the list
/// opens on the copy rather than on the session it was taken from.
fn fork_meta(dir: &Path, from: &SessionMeta) -> SessionMeta {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    let created = now.max(from.created.saturating_add(1));
    let title = from.title.trim();
    SessionMeta {
        id: free_id(dir, created),
        title: if title.is_empty() {
            "fork".to_string()
        } else {
            format!("{title} (fork)")
        },
        worktree: from.worktree.clone(),
        branch: from.branch.clone(),
        model: from.model.clone(),
        created,
    }
}

/// A file name in `dir` no session holds yet: `<created>-fork`, then `-2`, `-3` beside it.
fn free_id(dir: &Path, created: u64) -> String {
    let mut id = format!("{created}-fork");
    let mut copy = 1;
    while meta_path(dir, &id).exists() {
        copy += 1;
        id = format!("{created}-fork-{copy}");
    }
    id
}
