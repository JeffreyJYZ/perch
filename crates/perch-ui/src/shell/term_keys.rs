//! A press as the bytes a shell expects: the terminal's own key names and modifiers, and the
//! Control chord a real terminal sends as one byte.

use perch_term::{Key as TermKey, Mods as TermMods};
use rune::{Key, Mods};

/// `Ctrl` on a US layout clears the top three bits of a letter's ASCII code.
pub(super) fn control(character: char) -> u8 {
    let code = u32::from(character.to_ascii_lowercase());
    if (u32::from('a')..=u32::from('z')).contains(&code) {
        (code & 0x1f) as u8
    } else {
        code as u8
    }
}

/// The terminal's own key for a pressed key, and the modifiers held with it.
///
/// `None` for a key a terminal has no use for: the window's chords are handled before this, and a
/// key with neither text nor a VT encoding would only put a byte in a shell by accident.
pub(super) fn term_event(key: &Key, mods: Mods) -> Option<perch_term::KeyEvent> {
    let term = match key {
        Key::Enter => TermKey::Enter,
        Key::Tab => TermKey::Tab,
        Key::Backspace => TermKey::Backspace,
        Key::Delete => TermKey::Delete,
        Key::Insert => TermKey::Insert,
        Key::Escape => TermKey::Escape,
        Key::Home => TermKey::Home,
        Key::End => TermKey::End,
        Key::PageUp => TermKey::PageUp,
        Key::PageDown => TermKey::PageDown,
        Key::Left => TermKey::ArrowLeft,
        Key::Right => TermKey::ArrowRight,
        Key::Up => TermKey::ArrowUp,
        Key::Down => TermKey::ArrowDown,
        Key::Function(1) => TermKey::F1,
        Key::Function(2) => TermKey::F2,
        Key::Function(3) => TermKey::F3,
        Key::Function(4) => TermKey::F4,
        Key::Function(5) => TermKey::F5,
        Key::Function(6) => TermKey::F6,
        Key::Function(7) => TermKey::F7,
        Key::Function(8) => TermKey::F8,
        Key::Function(9) => TermKey::F9,
        Key::Function(10) => TermKey::F10,
        Key::Function(11) => TermKey::F11,
        Key::Function(12) => TermKey::F12,
        _ => return None,
    };
    let mut held = TermMods::empty();
    held.set(TermMods::SHIFT, mods.shift);
    held.set(TermMods::CTRL, mods.ctrl);
    held.set(TermMods::ALT, mods.alt);
    held.set(TermMods::SUPER, mods.logo);
    Some(perch_term::KeyEvent {
        key: term,
        mods: held,
    })
}
