//! The header row a new session answers with.
//!
//! Split from [`live`](super) for length only: the window appends this row to the session list
//! without reading the directory again, so the row and the index must come from the same session.

use crate::AppState;

/// The header of the session just opened: the row the window appends without reading the disk.
pub(super) fn row(
    app: &AppState,
    index: usize,
) -> std::result::Result<(usize, perch_store::SessionMeta), String> {
    let meta = app
        .active_session()
        .map(|session| session.meta.clone())
        .ok_or_else(|| "the session did not open".to_string())?;
    Ok((index, meta))
}
