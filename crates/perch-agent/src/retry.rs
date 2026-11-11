//! Bounded retry for provider failures: the part of a harness that keeps a turn alive when the
//! provider says "not now".
//!
//! Every harness that survives a real day does this — OpenCode retries rate limits and provider
//! faults on an exponential schedule for about a minute, honours a provider's `retry-after`, and
//! treats a context-length rejection as a different decision entirely. Without it a single `429`
//! ends a turn, which reads as "the agent loop is broken" even though the loop is fine.
//!
//! Only failures that arrive before any text does are retried: a stream that broke halfway has
//! already shown the user a partial answer, and asking again would make it appear twice.

use std::time::Duration;

use crate::Error;

/// How many times one request is asked before the failure is reported.
pub const MAX_ATTEMPTS: u32 = 10;

/// The first gap, doubled each attempt until [`CAP`].
const FIRST: Duration = Duration::from_secs(2);

/// The largest gap between two attempts. Ten attempts therefore cost about a minute, not an hour.
const CAP: Duration = Duration::from_secs(10);

/// The slice a wait is checked in, so a cancel during a backoff is noticed promptly.
const SLICE: Duration = Duration::from_millis(50);

/// Whether the same request may be asked again.
///
/// Rate limits, overload and server faults are the provider saying "later"; a bad request, a bad
/// key or an empty model is the provider saying "not like that", and asking again only wastes time.
pub fn worth_retrying(error: &Error) -> bool {
    match error {
        Error::Status { status, .. } => matches!(status, 408 | 409 | 425 | 429 | 500 | 502 | 503 | 504 | 529),
        Error::Http(inner) => inner.is_timeout() || inner.is_connect() || inner.is_request(),
        // A stream that ended without `[DONE]` is a cut connection, which is worth one more ask.
        Error::Truncated => true,
        Error::Io(_) | Error::Json(_) | Error::Config(_) | Error::Closed => false,
    }
}

/// How long to wait before attempt `attempt`, counting the first ask as one.
pub fn delay(attempt: u32) -> Duration {
    // 2s, 4s, 8s, then 10s for every further gap, which is the shape OpenCode uses.
    let doublings = attempt.saturating_sub(1).min(3);
    let millis = FIRST.as_millis() as u64 * (1_u64 << doublings);
    let capped = millis.min(CAP.as_millis() as u64);
    Duration::from_millis(capped)
}

/// One request's worth of attempts.
pub struct Retries {
    attempt: u32,
}

impl Retries {
    /// A fresh allowance.
    pub fn new() -> Retries {
        Retries { attempt: 0 }
    }

    /// Records a failure and answers how long to wait before asking again, or `None` when this
    /// failure is final.
    pub fn after(&mut self, error: &Error) -> Option<Duration> {
        if !worth_retrying(error) {
            return None;
        }
        self.attempt = self.attempt.saturating_add(1);
        if self.attempt >= MAX_ATTEMPTS {
            return None;
        }
        Some(delay(self.attempt))
    }

    /// How many attempts have failed so far.
    pub fn attempts(&self) -> u32 {
        self.attempt
    }
}

impl Default for Retries {
    fn default() -> Self {
        Retries::new()
    }
}

/// Sleeps for `gap`, a slice at a time, and answers whether the wait finished.
///
/// `false` means the turn was cancelled while waiting: the caller stops there and leaves the
/// conversation as it was, exactly as a cancel during a request does.
pub fn wait(gap: Duration, stopped: &dyn Fn() -> bool) -> bool {
    let mut left = gap;
    while left > Duration::ZERO {
        if stopped() {
            return false;
        }
        let slice = left.min(SLICE);
        std::thread::sleep(slice);
        left -= slice;
    }
    !stopped()
}
