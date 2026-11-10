//! The unambiguous VT sequences, used when the encoder is unavailable.

use libghostty_vt::key::{Key, Mods};

use super::character::character;
use crate::session::KeyEvent;

/// The unambiguous VT sequences, used when the encoder is unavailable.
///
/// This is what a terminal without any mode state can promise: Enter, Tab,
/// Backspace, Escape, the arrows in their default (CSI) form, and the plain
/// text of a printable key.
pub(super) fn literal(event: KeyEvent) -> Vec<u8> {
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
