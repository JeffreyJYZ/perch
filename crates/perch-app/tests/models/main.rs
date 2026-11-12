//! The model list and the per-session model, end to end over the app.
//!
//! Every listing here answers from a table through the app's own transport seam, so no socket is
//! opened and no provider is required. A keychain is never reached: the store is in memory.
//!
//! Split in two: what the provider's list becomes, and what picking one does to a session.

mod harness;
mod lazy;
mod lazy_shell;
mod list;
mod session;
