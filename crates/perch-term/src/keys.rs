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
    key::{Action, Encoder, Event, Key, Mods, OptionAsAlt},
};

use crate::session::KeyEvent;

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

/// The character this physical key produces on a US layout, before any Ctrl or
/// Meta transformation, which is exactly what [`Event::set_utf8`] documents.
fn character(event: KeyEvent) -> Option<char> {
    let base = match event.key {
        Key::A => 'a',
        Key::B => 'b',
        Key::C => 'c',
        Key::D => 'd',
        Key::E => 'e',
        Key::F => 'f',
        Key::G => 'g',
        Key::H => 'h',
        Key::I => 'i',
        Key::J => 'j',
        Key::K => 'k',
        Key::L => 'l',
        Key::M => 'm',
        Key::N => 'n',
        Key::O => 'o',
        Key::P => 'p',
        Key::Q => 'q',
        Key::R => 'r',
        Key::S => 's',
        Key::T => 't',
        Key::U => 'u',
        Key::V => 'v',
        Key::W => 'w',
        Key::X => 'x',
        Key::Y => 'y',
        Key::Z => 'z',
        Key::Digit0 => '0',
        Key::Digit1 => '1',
        Key::Digit2 => '2',
        Key::Digit3 => '3',
        Key::Digit4 => '4',
        Key::Digit5 => '5',
        Key::Digit6 => '6',
        Key::Digit7 => '7',
        Key::Digit8 => '8',
        Key::Digit9 => '9',
        Key::Space => ' ',
        Key::Backquote => '`',
        Key::Backslash => '\\',
        Key::BracketLeft => '[',
        Key::BracketRight => ']',
        Key::Comma => ',',
        Key::Equal => '=',
        Key::Minus => '-',
        Key::Period => '.',
        Key::Quote => '\'',
        Key::Semicolon => ';',
        Key::Slash => '/',
        _ => return None,
    };

    // Ctrl and Meta are folded by the encoder from the unmodified character, so
    // Shift only changes the text when nothing else is held.
    let folded = event.mods.intersects(Mods::CTRL | Mods::SUPER);
    if folded || !event.mods.contains(Mods::SHIFT) {
        return Some(base);
    }
    Some(shifted(base))
}

/// The unshifted key above its shifted twin, US layout.
fn shifted(base: char) -> char {
    match base {
        '0' => ')',
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '`' => '~',
        '\\' => '|',
        '[' => '{',
        ']' => '}',
        ',' => '<',
        '=' => '+',
        '-' => '_',
        '.' => '>',
        '\'' => '"',
        ';' => ':',
        '/' => '?',
        other => other.to_ascii_uppercase(),
    }
}

/// The unambiguous VT sequences, used when the encoder is unavailable.
///
/// This is what a terminal without any mode state can promise: Enter, Tab,
/// Backspace, Escape, the arrows in their default (CSI) form, and the plain
/// text of a printable key.
fn literal(event: KeyEvent) -> Vec<u8> {
    let mut out = Vec::new();
    if event.mods.contains(Mods::ALT) {
        out.push(0x1b);
    }
    match event.key {
        Key::Enter | Key::NumpadEnter => out.push(b'\r'),
        Key::Tab => out.push(b'\t'),
        Key::Backspace => out.push(0x7f),
        Key::Escape => out.push(0x1b),
        Key::ArrowUp => out.extend_from_slice(b"\x1b[A"),
        Key::ArrowDown => out.extend_from_slice(b"\x1b[B"),
        Key::ArrowRight => out.extend_from_slice(b"\x1b[C"),
        Key::ArrowLeft => out.extend_from_slice(b"\x1b[D"),
        Key::Home => out.extend_from_slice(b"\x1b[H"),
        Key::End => out.extend_from_slice(b"\x1b[F"),
        Key::Delete => out.extend_from_slice(b"\x1b[3~"),
        Key::PageUp => out.extend_from_slice(b"\x1b[5~"),
        Key::PageDown => out.extend_from_slice(b"\x1b[6~"),
        Key::Insert => out.extend_from_slice(b"\x1b[2~"),
        Key::F1 => out.extend_from_slice(b"\x1bOP"),
        Key::F2 => out.extend_from_slice(b"\x1bOQ"),
        Key::F3 => out.extend_from_slice(b"\x1bOR"),
        Key::F4 => out.extend_from_slice(b"\x1bOS"),
        key => {
            if key == Key::Space {
                out.push(b' ');
                return out;
            }
            let Some(text) = character(KeyEvent {
                key,
                mods: event.mods,
            }) else {
                return out;
            };
            if event.mods.contains(Mods::CTRL) {
                out.push(control(text));
            } else {
                let mut buffer = [0u8; 4];
                out.extend_from_slice(text.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    out
}

/// `Ctrl` on a US layout clears the top three bits of the ASCII code, which is
/// how `Ctrl+@` through `Ctrl+_` give 0x00 through 0x1f.
fn control(text: char) -> u8 {
    let code = u32::from(text);
    let folds = (u32::from('@')..=u32::from('_')).contains(&code)
        || (u32::from('a')..=u32::from('z')).contains(&code);
    if folds {
        (code & 0x1f) as u8
    } else {
        code as u8
    }
}
