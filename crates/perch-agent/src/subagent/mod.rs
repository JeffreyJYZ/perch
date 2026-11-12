//! The `task` tool: one child turn, run while the parent waits for it.
//!
//! A delegation is an ordinary tool call, so the model needs no second protocol: it writes a
//! brief, a child runs its own loop, and the child's answer comes back as the call's result. The
//! round limit and the loop belong to the [`Delegate`]; three things are settled here, because they
//! are the places a child can hurt the turn that started it:
//!
//! - **No recursion.** A registry that already holds `task` is refused, so the registry a child is
//!   built from cannot contain the tool that would let it start a child of its own. This is a
//!   structural refusal, not a counter that a caller could reset.
//! - **No writes.** The child is handed the parent's tools minus every one that asks a person, so
//!   it cannot edit a file while the person who would have approved that edit is waiting.
//! - **No unstated size.** The brief and the answer are both capped, and a cut is named in the text
//!   the model reads rather than made silently.
//!
//! Cancellation is the parent's: the tool hands the child the same [`Cancel`](crate::cancel::Cancel)
//! its own turn runs under, so stopping the turn stops the child instead of leaving it working for
//! nobody.

mod delegate;
mod limits;
mod registry;
mod text;
mod tool;

pub use delegate::Delegate;
pub use limits::{PROMPT_MAX_BYTES, RESULT_MAX_BYTES};
pub use tool::{TASK, Task};
