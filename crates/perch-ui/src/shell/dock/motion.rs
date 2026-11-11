//! What a new terminal frame does to the window: it lands, it does not glide.
//!
//! Rune animates a commit by default: an element that exists in the last frame and moves this
//! one travels there on a spring (`apply.rs` gates that on `!instant`). A terminal's cursor is
//! such an element — it moves one cell to the right on every echoed character — so a commit that
//! animates it makes the caret slide toward the text instead of jumping with the key.

use perch_term::replay;
use rune::{Env, Runtime};

use super::super::Shell;
use crate::shell::view;

/// The window the dock lays its grid out in.
const WINDOW: (f32, f32) = (1280.0, 820.0);

/// A laid-out, settled window over one tab showing `bytes` in a 20-column grid of `rows` lines.
fn settled_window(bytes: &[u8], rows: u16) -> Runtime<Shell> {
    let mut runtime = Runtime::new(
        Shell::idle(Vec::new(), 0).with_frame("t", replay(bytes, 20, rows)),
        view,
        WINDOW,
        Env::new(true, 1.0),
    );
    runtime.frame(0.0);
    // The window's own entrance is motion of its own; let it finish, so what follows is only the
    // dock's change.
    for _ in 0..4 {
        if !runtime.frame(0.5) {
            break;
        }
    }
    assert!(
        !runtime.frame(1.0 / 60.0),
        "the window is at rest before the terminal frame changes"
    );
    runtime
}

#[test]
fn an_echoed_character_lands_the_cursor_instead_of_gliding_it() {
    let mut runtime = settled_window(b"hi", 4);
    // The next frame is what a shell's echo produces: one more character, the caret one cell on.
    runtime.update(|shell, _| {
        shell.set_frame(0, replay(b"hi!", 20, 4));
    });

    let animating = runtime.frame(1.0 / 60.0);
    assert!(
        !animating,
        "the dock's grid must land its new frame at once: a spring here is a caret that slides"
    );
}
