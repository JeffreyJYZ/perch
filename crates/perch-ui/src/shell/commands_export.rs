//! `/export`: one session's transcript, written as text under the data directory.
//!
//! The file is the column as a person reads it, earliest item first, so its first line is the
//! first thing the session said. Nothing is sent anywhere: the path is returned and the command
//! says it in the transcript.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use perch_store::SessionMeta;

use crate::transcript::{Item, Transcript};

/// Writes `transcript` to `<dir>/export-<id>.txt` and answers the path.
pub(super) fn export(
    dir: &Path,
    meta: &SessionMeta,
    transcript: &Transcript,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("the data directory: {error}"))?;
    let path = dir.join(format!("export-{}.txt", meta.id));
    std::fs::write(&path, text(transcript))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

/// The transcript as plain lines, one item after another.
#[must_use]
pub fn text(transcript: &Transcript) -> String {
    let mut out = String::new();
    for item in transcript.items() {
        let _ = writeln!(out, "{}", line(item));
    }
    out
}

/// One item's line: who said it, and what was said, with a call's status in its own brackets.
fn line(item: &Item) -> String {
    match item {
        Item::User { text } => format!("you: {text}"),
        Item::Assistant { text } => format!("perch: {text}"),
        Item::Reasoning { text } => format!("thinking: {text}"),
        Item::Note(text) => text.clone(),
        Item::Tool {
            name,
            status,
            output,
            target,
            ..
        } => {
            let what = target
                .as_ref()
                .map_or_else(String::new, |target| format!(" ({target})"));
            if output.is_empty() {
                format!("[{name}{what}: {status}]")
            } else {
                format!("[{name}{what}: {status}] {output}")
            }
        }
        Item::Summary {
            tools,
            failed,
            tokens,
        } => {
            let spent = tokens.map_or_else(String::new, |tokens| format!(", {tokens} tokens"));
            format!("[turn: {tools} calls, {failed} failed{spent}]")
        }
    }
}

#[cfg(test)]
mod tests {
    use perch_agent::AgentEvent;
    use perch_store::SessionMeta;

    use super::{export, text};
    use crate::transcript::Transcript;

    fn meta() -> SessionMeta {
        SessionMeta {
            id: "s1".to_string(),
            title: "s".to_string(),
            worktree: std::env::temp_dir(),
            branch: String::new(),
            model: String::new(),
            created: 0,
        }
    }

    #[test]
    fn the_first_line_is_the_first_item_and_the_file_is_under_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let mut transcript = Transcript::default();
        transcript.push_user("what is this?");
        transcript.apply(&AgentEvent::TextDelta("a perch".to_string()));

        let path = export(dir.path(), &meta(), &transcript).unwrap();
        assert_eq!(path.parent(), Some(dir.path()));
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            written.lines().next(),
            Some("you: what is this?"),
            "{written}"
        );
        assert!(written.contains("perch: a perch"), "{written}");
    }

    #[test]
    fn an_empty_transcript_is_an_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(text(&Transcript::default()), "");
        let path = export(dir.path(), &meta(), &Transcript::default()).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "");
    }
}
