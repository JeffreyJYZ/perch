//! Task 13: tool dispatch, approvals, and the question tool.
//!
//! Every test also pins the call id on the tool message it gets back, because the provider
//! addresses each result to the assistant's call by that id and a wrong one is rejected.
//!
//! One child per behaviour: a call the user answers, a call that fails, and the question tool.

mod approval;
mod harness;
mod preview;
mod question;
mod results;
