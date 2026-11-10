//! The bytes a key press becomes, including the ones only the terminal can decide.

use perch_term::{
    Key, KeyEvent, Mods, Terminal, TerminalOptions, encode_key, encode_key_for_terminal,
};

#[test]
fn control_and_arrow_keys_are_encoded() {
    // The plan spells these `Key::Char('c')` and `Mods::NONE`; the library has
    // a physical-key enum and bitflags instead, so `C` plus CTRL is the same
    // press expressed in the types that exist.
    assert_eq!(
        encode_key(KeyEvent {
            key: Key::C,
            mods: Mods::CTRL,
        }),
        b"\x03".to_vec()
    );
    assert_eq!(
        encode_key(KeyEvent::plain(Key::ArrowUp)),
        b"\x1b[A".to_vec()
    );
    assert_eq!(encode_key(KeyEvent::plain(Key::Enter)), b"\r".to_vec());
}

#[test]
fn modifier_and_function_keys_are_encoded() {
    let cases = [
        (Key::ArrowUp, Mods::CTRL, b"\x1b[1;5A".to_vec()),
        (Key::B, Mods::ALT, b"\x1bb".to_vec()),
        (Key::Tab, Mods::SHIFT, b"\x1b[Z".to_vec()),
        (Key::F5, Mods::empty(), b"\x1b[15~".to_vec()),
        (Key::A, Mods::empty(), b"a".to_vec()),
        (Key::A, Mods::SHIFT, b"A".to_vec()),
    ];
    for (key, mods, expected) in cases {
        let encoded = encode_key(KeyEvent { key, mods });
        eprintln!("encode {key:?} {mods:?} -> {encoded:?}");
        assert_eq!(encoded, expected, "{key:?} with {mods:?}");
    }
}

#[test]
fn application_cursor_mode_changes_the_arrow_encoding() {
    let mut terminal = Terminal::new(TerminalOptions {
        cols: 80,
        rows: 24,
        max_scrollback: 0,
    })
    .expect("create a terminal");
    // DECCKM, which a full-screen program turns on and which only the terminal
    // knows about: without reading it back, `Up` is the wrong three bytes.
    terminal.vt_write(b"\x1b[?1h");

    let encoded = encode_key_for_terminal(KeyEvent::plain(Key::ArrowUp), &terminal);
    eprintln!("encode Up with DECCKM -> {encoded:?}");
    assert_eq!(encoded, b"\x1bOA".to_vec());
}
