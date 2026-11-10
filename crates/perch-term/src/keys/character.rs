//! The character a physical key produces on a US layout.

use libghostty_vt::key::{Key, Mods};

use crate::session::KeyEvent;

/// The character this physical key produces on a US layout, before any Ctrl or
/// Meta transformation, which is exactly what `Event::set_utf8` documents.
pub(super) fn character(event: KeyEvent) -> Option<char> {
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
