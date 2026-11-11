//! The `task` tool: one child turn, its caps, and what the child may touch.
//!
//! One child per behaviour: the result it answers with, the limits it refuses on, and the registry
//! the delegate is handed. Every test drives the tool itself, so what is asserted is the result a
//! model would read rather than the shape of the struct behind it.

mod harness;
mod limits;
mod result;
mod tools;
