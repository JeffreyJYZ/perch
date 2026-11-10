//! A cancel: it must end the turn exactly once, and stop it mid-stream.

use std::sync::atomic::Ordering;

use perch_agent::{AgentEvent, EndReason};

use crate::harness::{EVENT_TIMEOUT, agent_at, answer, collect_from, until_call};

#[test]
fn a_cancel_while_waiting_for_a_tool_result_asks_nothing_further() {
    let fixture = crate::support::serve("tests/fixtures/sse_simple.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    handle.prompt("go".into());
    let (earlier, call) = until_call(&events);

    handle.cancel();
    answer(&handle, std::slice::from_ref(&call), "fn main() {}");
    let seen = collect_from(&events, earlier);

    assert_eq!(seen.ends().len(), 1, "a cancelled turn ends exactly once");
    assert_eq!(seen.end(), &EndReason::Cancelled);
    assert_eq!(
        seen.tool_calls().len(),
        1,
        "no call was made after the cancel"
    );
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        1,
        "a cancelled turn asks the model nothing further"
    );
}

#[test]
fn cancel_yields_exactly_one_cancelled_end() {
    let fixture = crate::support::serve("tests/fixtures/sse_slow.txt");
    let (handle, events) = agent_at(&fixture.base_url);
    handle.prompt("go".into());
    // Wait for the reply to start before cancelling. A cancel that landed before the request
    // went out would pass this test without ever testing a stop mid-stream, which is the case
    // that actually has to work.
    let first = events
        .recv_timeout(EVENT_TIMEOUT)
        .expect("the reply should start streaming");
    assert!(
        matches!(&first, AgentEvent::TextDelta(_)),
        "expected text before the cancel, got {first:?}"
    );
    handle.cancel();
    let seen = collect_from(&events, vec![first]);
    assert_eq!(seen.ends().len(), 1);
    assert_eq!(seen.end(), &EndReason::Cancelled);
    // The fixture is paced, so a turn that ignored the cancel would consume every event before
    // ending. Seeing the server still holding events back is what proves the stop was mid-stream.
    let written = fixture.written.load(Ordering::SeqCst);
    assert!(
        written < fixture.total,
        "the cancel landed only after all {written} events were served"
    );
}
