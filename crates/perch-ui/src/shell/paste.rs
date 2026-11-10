//! A paste from the app root: Rune reads the clipboard at the root only, so a ⌘V that no text
//! field took arrives here and goes to the dock, which is where the window's typing lands.
//!
//! The dock's own ⌘V already reaches the same tab method; this is the other door into it, and
//! both end in [`TerminalTab::paste`], so the bracketed-paste rule is applied once.

use rune::Tx;

use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// Sends `text` to the shown terminal tab as a paste. False when no tab there can take one:
    /// no tab at all, or one whose shell has stopped.
    pub fn paste(&mut self, text: &str) -> bool {
        let index = self.terminal;
        self.terminals
            .get_mut(index)
            .is_some_and(|tab| tab.paste(text))
    }
}

/// The app root's paste handler: everything the clipboard held that no text field took.
///
/// A paste with no shell to land in is dropped rather than turned into a composer draft: the
/// window said nothing was running, and a paste is a keystroke, not a file.
pub fn root_paste<L: Live>(shell: &mut Shell<L>, text: &str, _: &mut Tx) {
    shell.paste(text);
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    use perch_term::{Frame, KeyEvent as TermEvent, Mode, PASTE_END, PASTE_START, replay};
    use rune::Tx;

    use super::{Shell, root_paste};
    use crate::shell::terminal::{TermIo, TerminalTab};

    /// A terminal thread that answers without a pty: what the dock wrote to it.
    #[derive(Clone, Default)]
    struct Fake {
        written: Arc<Mutex<Vec<u8>>>,
        bracketed: bool,
    }

    impl TermIo for Fake {
        fn write(&self, bytes: &[u8]) {
            self.written.lock().unwrap().extend_from_slice(bytes);
        }
        fn key(&self, _: TermEvent) -> Vec<u8> {
            Vec::new()
        }
        fn mode(&self, mode: Mode) -> bool {
            mode == Mode::BRACKETED_PASTE && self.bracketed
        }
        fn scroll(&self, _: usize) -> bool {
            true
        }
        fn resize(&self, _: u16, _: u16) {}
        fn exited(&mut self) -> bool {
            false
        }
    }

    /// A window whose dock shows one tab over `fake`.
    fn docked(fake: &Fake) -> Shell {
        let (_, frames) = mpsc::channel::<Frame>();
        let frame = replay(b"$ ", 20, 4);
        let mut shell = Shell::idle(Vec::new(), 0);
        shell
            .terminals
            .push(TerminalTab::over(Box::new(fake.clone()), frames, (20, 4)));
        shell.set_frame(0, frame);
        shell
    }

    #[test]
    fn a_root_paste_lands_in_the_shown_tab_as_the_dock_sends_it() {
        let fake = Fake {
            bracketed: true,
            ..Fake::default()
        };
        let mut shell = docked(&fake);

        root_paste(&mut shell, "cargo test\n", &mut Tx::default());

        let mut expected = PASTE_START.to_vec();
        expected.extend_from_slice(b"cargo test\n");
        expected.extend_from_slice(PASTE_END);
        assert_eq!(
            *fake.written.lock().unwrap(),
            expected,
            "the root paste is the dock's paste: markers and all"
        );
    }

    #[test]
    fn a_root_paste_with_no_shell_to_land_in_does_nothing() {
        let mut empty = Shell::idle(Vec::new(), 0);
        assert!(!empty.paste("hello"), "no tab means nothing took the paste");

        // A tab whose shell has stopped takes no paste either, and says so by its answer.
        let fake = Fake::default();
        let mut shell = docked(&fake);
        shell.terminals[0] = TerminalTab::failed("t", "the shell exited");
        assert!(!shell.paste("hello"));
        assert!(fake.written.lock().unwrap().is_empty());
    }
}
