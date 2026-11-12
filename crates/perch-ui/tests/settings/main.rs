//! The settings screen: what it refuses, what it draws, and the two ways into it.
//!
//! No keychain is involved anywhere here: the panel is opened over [`Idle`] or a test app that
//! keeps its "key" in memory, so a draft is validated, drawn and saved without a provider. The
//! key's round trip through a real store is proved in `perch-app`'s tests.

mod compact;
mod draw;
mod entry;
mod fake;
mod harness;
mod keybinds;
mod meter;
mod picker;
mod profiles;
mod save;
mod tab;
mod theme;
mod validate;
