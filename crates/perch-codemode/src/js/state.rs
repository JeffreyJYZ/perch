//! The one execution a call is running, reachable from the host functions.
//!
//! boa's natives get `&mut Context` and nothing else, and this crate's per-call state borrows a
//! host, a budget and an output buffer, so none of it can live in a GC object. It lives on the
//! thread for the length of one call instead: entered before the program runs, taken back when it
//! returns. A program cannot reach `execute` itself — the host snapshot never holds it — and even
//! a nested call restores the outer state in order.

use std::cell::RefCell;
use std::sync::Arc;

use perch_tools::ToolCtx;

use crate::budget::Budget;
use crate::diag::Stop;
use crate::host::ToolHost;
use crate::output::Out;

/// Everything one execution is spending, and the stop it already recorded.
pub(super) struct State {
    pub host: Arc<dyn ToolHost>,
    pub ctx: ToolCtx,
    pub budget: Budget,
    pub out: Out,
    pub stop: Option<Stop>,
}

thread_local! {
    static CURRENT: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Makes `state` the execution in progress, answering whatever was there before.
pub(super) fn enter(state: State) -> Option<State> {
    CURRENT.with(|slot| slot.borrow_mut().replace(state))
}

/// Ends the execution in progress, answering it and putting `previous` back.
pub(super) fn leave(previous: Option<State>) -> Option<State> {
    CURRENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        let current = slot.take();
        *slot = previous;
        current
    })
}

/// Runs `f` against the execution in progress, or answers `None` when there is none.
pub(super) fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    CURRENT.with(|slot| slot.borrow_mut().as_mut().map(f))
}
