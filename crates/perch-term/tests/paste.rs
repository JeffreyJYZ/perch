//! Bracketed paste: the markers, the literal payload, and the one sequence that must not pass.

use perch_term::{PASTE_END, PASTE_START, encode_paste};

/// The markers a paste may carry, in each spelling a parser accepts.
const MARKERS: [&[u8]; 4] = [b"\x1b[201~", b"\x1b[200~", b"\x1b[201;1~", b"\x9b201~"];

#[test]
fn the_payload_is_untouched_when_the_shell_did_not_enable_the_mode() {
    // A shell that never sent DECSET 2004 does not understand the markers and would print
    // them, so the bytes go out exactly as the clipboard held them — newline and all.
    let payload = b"rm -rf /\nsecond line\tend";
    assert_eq!(encode_paste(payload, false), payload.to_vec());
}

#[test]
fn a_paste_is_wrapped_and_its_newline_stays_inside_the_bracket() {
    let wrapped = encode_paste(b"echo one\necho two", true);
    assert_eq!(wrapped, b"\x1b[200~echo one\necho two\x1b[201~".to_vec());
    // The newline is inside the markers, which is the whole point: the shell inserts it
    // instead of reading it as Enter, so a pasted command does not run by itself.
    let inside = inner(&wrapped);
    assert_eq!(inside, b"echo one\necho two");
    assert_eq!(inside.iter().filter(|byte| **byte == b'\n').count(), 1);
}

#[test]
fn no_marker_in_the_paste_can_close_or_reopen_the_bracket() {
    for marker in MARKERS {
        let mut payload = b"safe".to_vec();
        payload.extend_from_slice(marker);
        payload.extend_from_slice(b"rm -rf /\n");

        let wrapped = encode_paste(&payload, true);
        assert!(wrapped.starts_with(PASTE_START), "{marker:?}");
        assert!(wrapped.ends_with(PASTE_END), "{marker:?}");
        let inside = inner(&wrapped);
        for other in MARKERS {
            assert!(
                !contains(inside, other),
                "{marker:?} left {other:?} inside the bracket: {inside:?}"
            );
        }
        // The command's own bytes survive: the paste is neutralised, not deleted.
        assert!(contains(inside, b"rm -rf /\n"), "{marker:?}: {inside:?}");
    }
}

/// The bytes between the markers this crate writes.
fn inner(wrapped: &[u8]) -> &[u8] {
    &wrapped[PASTE_START.len()..wrapped.len() - PASTE_END.len()]
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
