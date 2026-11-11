//! perch-ui: the window's layout and the painter's view of a terminal frame.
//!
//! [`shell`] is the window itself — one left column of sessions over files, the transcript in
//! the centre, and the terminal dock under the transcript only.

pub mod files;
pub mod kit;
pub mod plugin_panel;
pub mod preview;
pub mod settings;
pub mod shell;
pub mod terminal_view;
pub mod transcript;

pub use files::{FilePanel, TreeRow, files_view, status_word};
pub use plugin_panel::{panel_lines, panel_view};
pub use preview::{preview_lines, preview_view};
pub use settings::{Field, Invalid, SettingsDraft, THEMES};
pub use shell::{COLUMN, Shell, TerminalTab};
pub use terminal_view::{RowRun, paint_rows};
pub use transcript::{
    COLLAPSED_LINES, Item, ToolStatus, Transcript, collapsed_output, transcript_view,
};
