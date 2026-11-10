//! Turning a key press into the bytes a shell expects.
//!
//! [`libghostty_vt::key::Encoder`] owns the hard part: which modifier set a
//! terminal reports in the `1;{mods}` CSI parameter, the DECCKM application
//! cursor sequences, the DECNKM keypad set, Kitty keyboard protocol flags and
//! `modifyOtherKeys`. It also needs a logical key plus the text the layout
//! produced, so a physical key and a modifier bitmask are all this module has
//! to supply.
//!
//! Printable characters go through [`Event::set_utf8`] with the character the
//! key produces on a US layout; everything else is encoded from the physical
//! key alone. [`literal`] is the fallback for the case where the encoder
//! cannot be created or writes nothing, so a dead encoder degrades to the
//! unambiguous VT sequences rather than swallowing the keystroke.

use libghostty_vt::{
    Terminal,
    key::{Action, Encoder, Event, OptionAsAlt},
};

use crate::session::KeyEvent;

use character::character;
use literal::literal;

mod character;
mod literal;

/// Encode a key press without a terminal, so cursor keys use the default
/// (non-application) form.
#[must_use]
pub fn encode_key(event: KeyEvent) -> Vec<u8> {
    let Ok(mut encoder) = Encoder::new() else {
        return literal(event);
    };
    configure(&mut encoder);
    encode_with(&mut encoder, event)
}

/// Encode a key press against a live terminal's modes.
///
/// This is the accurate path: a full-screen program that enabled application
/// cursor keys, application keypad, `modifyOtherKeys` or the Kitty keyboard
/// protocol changes what the same physical key must send, and only the
/// terminal knows which of those are on.
#[must_use]
pub fn encode_key_for_terminal(event: KeyEvent, terminal: &Terminal<'_, '_>) -> Vec<u8> {
    let Ok(mut encoder) = Encoder::new() else {
        return literal(event);
    };
    encoder.set_options_from_terminal(terminal);
    configure(&mut encoder);
    encode_with(&mut encoder, event)
}

/// The options this app always wants, applied after the terminal's own modes.
///
/// `Option` is Alt on this platform, and a terminal that swallows it is a
/// terminal where `Option+b` types a `b` instead of moving back a word. Ghostty
/// makes the same call with `macos-option-as-alt = true`.
fn configure(encoder: &mut Encoder<'_>) {
    encoder.set_macos_option_as_alt(OptionAsAlt::True);
    encoder.set_alt_esc_prefix(true);
}

fn encode_with(encoder: &mut Encoder<'_>, event: KeyEvent) -> Vec<u8> {
    let Ok(mut key_event) = Event::new() else {
        return literal(event);
    };
    key_event
        .set_action(Action::Press)
        .set_key(event.key)
        .set_mods(event.mods);
    if let Some(text) = character(event) {
        // `set_utf8` must see the character before Ctrl or Meta fold it, so a
        // Ctrl+C still passes `c` and the encoder derives 0x03 from the mods.
        key_event.set_utf8(Some(text.to_string()));
    }

    let mut out = Vec::with_capacity(8);
    match encoder.encode_to_vec(&key_event, &mut out) {
        // An unmodified modifier or a key the encoder declines produces no
        // bytes, which is a real answer, not a failure — but a key that carries
        // text and still produced nothing is, and the literal form rescues it.
        Ok(()) if !out.is_empty() || character(event).is_none() => out,
        Ok(()) | Err(_) => literal(event),
    }
}
