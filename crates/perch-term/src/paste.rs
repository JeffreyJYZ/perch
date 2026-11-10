//! Bracketed paste: wrapping pasted text so a shell inserts it instead of running it.
//!
//! A terminal that pastes raw bytes hands the shell a stream it cannot tell from typing, so a
//! clipboard holding `rm -rf /\n` runs the command the moment it is pasted. DEC mode 2004 is
//! the fix: the shell says it understands the markers, the terminal wraps the text in
//! `CSI 200 ~` and `CSI 201 ~`, and everything between them is inserted literally, newline
//! included.
//!
//! [`encode_paste`] therefore has two jobs. It wraps the payload when the shell asked for the
//! mode, and it removes any marker the payload carries itself: a clipboard that ends in
//! `CSI 201 ~` would otherwise close the bracket early and let the rest of the paste arrive as
//! keystrokes, which is the injection the mode exists to stop.
//!
//! `libghostty_vt::paste::encode` does neither of those two things exactly: it rewrites the
//! payload even when bracketed mode is off (newlines become carriage returns, every unsafe
//! control byte becomes a space), so it cannot honour "unchanged when the mode is off", and it
//! is an FFI call that needs a buffer sized by retry.

/// The sequence sent before pasted text when the shell enabled bracketed paste.
pub const PASTE_START: &[u8] = b"\x1b[200~";

/// The sequence sent after pasted text when the shell enabled bracketed paste.
pub const PASTE_END: &[u8] = b"\x1b[201~";

/// The bytes to write to the shell for a paste of `bytes`.
///
/// With `enabled` false the payload goes out untouched: a shell that never asked for bracketed
/// paste does not understand the markers and would print them. With it true the payload is
/// wrapped, and any bracketed-paste marker inside it is dropped, so the paste can only end at
/// the end this function writes.
#[must_use]
pub fn encode_paste(bytes: &[u8], enabled: bool) -> Vec<u8> {
    if !enabled {
        return bytes.to_vec();
    }
    let mut out = Vec::with_capacity(bytes.len() + PASTE_START.len() + PASTE_END.len());
    out.extend_from_slice(PASTE_START);
    copy_without_markers(bytes, &mut out);
    out.extend_from_slice(PASTE_END);
    out
}

/// Copy `bytes` into `out`, skipping every bracketed-paste marker it contains.
fn copy_without_markers(bytes: &[u8], out: &mut Vec<u8>) {
    let mut index = 0;
    while index < bytes.len() {
        match marker_len(&bytes[index..]) {
            Some(len) => index += len,
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
}

/// How many bytes a bracketed-paste marker at the start of `bytes` takes, if one starts there.
///
/// Both spellings of CSI count, `ESC [` and the single byte `0x9b`: a parser accepts either,
/// and a marker only one of them recognises is still a marker.
fn marker_len(bytes: &[u8]) -> Option<usize> {
    let params_at = match bytes {
        [0x1b, b'[', ..] => 2,
        [0x9b, ..] => 1,
        _ => return None,
    };
    let mut index = params_at;
    while let Some(byte) = bytes.get(index) {
        match *byte {
            b'0'..=b'9' | b';' => index += 1,
            b'~' if is_paste_marker(&bytes[params_at..index]) => return Some(index + 1),
            _ => return None,
        }
    }
    None
}

/// Whether a CSI parameter string names bracketed paste: `200` or `201`, then anything.
fn is_paste_marker(params: &[u8]) -> bool {
    let first = params
        .split(|byte| *byte == b';')
        .next()
        .unwrap_or_default();
    first == b"200" || first == b"201"
}
