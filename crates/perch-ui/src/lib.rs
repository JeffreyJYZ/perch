//! perch-ui: the window's layout and the painter's view of a terminal frame.
//!
//! [`shell`] is the window itself — one left column of sessions over files, the transcript in
//! the centre, and the terminal dock under the transcript only.

pub mod shell;
pub mod terminal_view;

pub use shell::Shell;
pub use terminal_view::{RowRun, paint_rows};
