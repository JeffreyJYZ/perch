//! The window's wiring, driven through a laid-out runtime: a file row's press fills the preview
//! and the preview's button hands the path to the app's editor, and Enter on a focused tool card
//! opens it — while Enter with nothing focused opens nothing.
//!
//! The editor is a fake, so no test starts a command; the app's own `Zed` is the only place an
//! `open -a` can run.

mod answer;
mod card;
mod harness;
mod preview;
