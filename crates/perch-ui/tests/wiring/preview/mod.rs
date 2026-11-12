//! The file panel's wiring: a row's press, the preview it puts in the centre column, and the
//! handoff it offers.
//!
//! The preview replaced the transcript's column, so these tests also pin the window's shape: the
//! preview and the tree never share pixels, the sidebar's own regions never do, and leaving the
//! preview brings the transcript back with the file still selected.

mod handoff;
mod header;
mod placement;
mod probe;
mod transcript;
