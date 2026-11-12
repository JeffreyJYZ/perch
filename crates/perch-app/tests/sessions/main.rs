//! The session list as the window builds it: what is read from disk, what a press on a row does,
//! starting a second session on the directory already open, and what a row says when its worktree
//! has gone.
//!
//! Nothing here reaches a provider: a session is opened, listed and switched, and no turn is sent
//! to the base URL — which is why it can be a dead port.

mod branch;
mod commands;
mod fork;
mod harness;
mod list;
mod missing;
mod new;
