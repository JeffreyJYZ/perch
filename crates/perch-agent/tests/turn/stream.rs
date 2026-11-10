//! A stream that stops without finishing is an errored turn, and it says why.

use perch_agent::EndReason;

use crate::harness::{agent_against, collect};

#[test]
fn a_cut_stream_ends_errored_with_the_reason() {
    let (handle, events) = agent_against("tests/fixtures/sse_cut.txt");
    handle.prompt("go".into());
    let seen = collect(&events);
    assert_eq!(seen.ends().len(), 1);
    match seen.end() {
        EndReason::Errored(reason) => {
            assert!(!reason.is_empty(), "an errored turn must say why it failed");
        }
        other => panic!("expected the cut stream to end errored, got {other:?}"),
    }
}
