//! The theme library: three anchors in a file, a refusal that says why, and the size scale.
//!
//! Every test reads from its own temporary themes directory, so nothing here touches the themes a
//! person actually has in `$HOME`.

mod anchors;
mod library;
mod size;
