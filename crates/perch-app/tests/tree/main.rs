//! The shell's acceptance tests, all through the accessibility tree: a tab switch moves the
//! active session, the window names every region the layout promises, and the regions the
//! composed window reads from — a worktree, a transcript and a terminal frame — reach the tree
//! they are drawn into and nowhere else.

mod dock;
mod empty;
mod harness;
mod panels;
mod regions;
