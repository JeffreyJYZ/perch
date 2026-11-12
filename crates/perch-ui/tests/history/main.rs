//! Prompt history: Up and Down over what a worktree has sent, frecent first and capped.
//!
//! `recall` is the walk itself; `persist` is the file it reads and writes across restarts.

mod harness;
mod persist;
mod recall;
