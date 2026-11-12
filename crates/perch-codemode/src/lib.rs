//! Code Mode: one `execute` tool that runs a JavaScript program which calls the other tools.
//!
//! # Why this exists
//!
//! A turn that reads four files and greps twice is five tool calls, five round trips and five
//! copies of everything they returned in the conversation. `execute` turns that into one call:
//! the model writes a program, the program calls the tools, and only what the program prints
//! reaches the transcript. The registry is untouched — a program calls a tool by the name the
//! registry already dispatches, so a plugin or MCP tool is reachable here the moment it is
//! reachable at all, and nothing has to know the set of tools in advance.
//!
//! The language is real JavaScript, run by [`boa_engine`], with `tools` and `print` bound into the
//! global scope.
//!
//! # What a caller must wire
//!
//! ```ignore
//! // `execute` must not be inside the registry its host reads, or the two hold each other.
//! let host = Arc::new(RegistryHost::new(registry.clone()));
//! registry.register(Arc::new(Execute::new(host).cancelled_by(cancel)));
//! ```
//!
//! # The security boundary
//!
//! A tool whose [`Tool::needs_approval`](perch_tools::Tool::needs_approval) is true is refused
//! inside a program, always, and twice: the host function refuses before it charges or calls, and
//! [`RegistryHost`] refuses again if anything drives it directly. The refusal is a thrown `Error`,
//! so a program that reaches for `write` learns why and can print it.
//!
//! # The budgets, and how honestly they bind
//!
//! Calls and output are charged inside the host functions, before the call runs or the bytes are
//! appended, so nothing can spend past a cap. The clock is checked at those same points, which
//! covers every program that does its work through a tool. A program that only computes — a
//! `while (true) {}` — never reaches a host function, and boa 0.22 offers no interrupt hook, so
//! that case is bounded by [`STEPS`]: a loop-iteration allowance injected through boa's runtime
//! limits that stands in for the clock and is reported as the wall-clock budget. The test that
//! proves it runs under a hard thread timeout so a wrong mechanism fails loudly instead of
//! wedging the suite.

mod budget;
mod diag;
mod execute;
mod host;
mod js;
mod memory;
mod output;
mod registry_host;

pub use budget::{CALLS, Cancel, Limits, OUTPUT, STEPS, WALL, Watch};
pub use diag::{Hit, Stop};
pub use execute::{DESCRIPTION, EXECUTE, Execute};
pub use host::ToolHost;
pub use memory::MemoryHost;
pub use registry_host::RegistryHost;
