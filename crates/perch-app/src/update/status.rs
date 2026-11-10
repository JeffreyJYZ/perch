//! What the update thread last found, said as the one line the window draws.
//!
//! The line is the whole contract with the window: "no update", "update X available" with the
//! release's notes, where a staged file landed, or why the check failed. Nothing here downloads
//! anything and nothing here installs anything.

use std::path::PathBuf;

use perch_plugins::{PanelEntry, PanelModel, PanelSection};
use perch_update::Version;

use super::{OWNER, STAGE};

/// The state of the one update check this app runs.
///
/// A worker exists only when a manifest URL is configured, so there is no "off" state here: a
/// window with no URL has no worker, and therefore no panel and no line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// The request is out and has not answered yet.
    Checking,
    /// The manifest is not newer than this build.
    Current,
    /// A newer release stands, and the person may ask for its archive.
    Offer { version: Version, notes: String },
    /// The archive matched its checksum and is on disk at `path`.
    Staged { version: Version, path: PathBuf },
    /// The check or the download failed, in words for the column.
    Failed(String),
}

impl Status {
    /// The line the window shows, in words.
    pub fn line(&self) -> String {
        match self {
            Status::Checking => "checking for updates".to_string(),
            Status::Current => "no update".to_string(),
            Status::Offer { version, notes } => offer_line(version, notes),
            Status::Staged { version, path } => format!(
                "update {version} staged at {}; installing it is a manual step after perch quits, \
                 because the bundle being replaced is this running app",
                path.display()
            ),
            Status::Failed(why) => format!("update check failed: {why}"),
        }
    }

    /// The panel the column draws: the line, and the download button while an offer stands.
    pub fn panel(&self) -> PanelEntry {
        let mut sections = vec![PanelSection::Text(self.line())];
        if matches!(self, Status::Offer { .. }) {
            sections.push(PanelSection::Button {
                id: STAGE.to_string(),
                label: "Download".to_string(),
            });
        }
        PanelEntry {
            plugin: OWNER.to_string(),
            model: PanelModel {
                title: "Update".to_string(),
                sections,
            },
        }
    }
}

/// One line, never a paragraph: a manifest's notes are free text, and a newline in them would
/// otherwise push the button below the fold and make "one line" a lie.
fn offer_line(version: &Version, notes: &str) -> String {
    let notes = notes.split_whitespace().collect::<Vec<_>>().join(" ");
    if notes.is_empty() {
        format!("update {version} available")
    } else {
        format!("update {version} available: {notes}")
    }
}
