use std::{cell::RefCell, rc::Rc, sync::mpsc};

use perch_term::{Frame, KeyEvent as TermEvent, Mode, PASTE_END, PASTE_START, replay};
use rune::{Key, KeyEvent, Mods};

use super::{Session, TermIo, TerminalTab};

/// A terminal thread that answers without a pty: what it was written and scrolled to, and
/// whether it says it enabled a mode.
#[derive(Clone, Default)]
struct Fake {
    written: Rc<RefCell<Vec<u8>>>,
    scrolled: Rc<RefCell<Vec<usize>>>,
    bracketed: bool,
}

impl TermIo for Fake {
    fn write(&self, bytes: &[u8]) {
        self.written.borrow_mut().extend_from_slice(bytes);
    }
    fn key(&self, _: TermEvent) -> Vec<u8> {
        Vec::new()
    }
    fn mode(&self, mode: Mode) -> bool {
        mode == Mode::BRACKETED_PASTE && self.bracketed
    }
    fn scroll(&self, offset: usize) -> bool {
        self.scrolled.borrow_mut().push(offset);
        true
    }
    fn resize(&self, _cols: u16, _rows: u16) {}
    fn exited(&mut self) -> bool {
        false
    }
}

/// A tab over `fake`, showing a `rows`-row frame so a page has a height to measure.
fn tab(fake: &Fake, rows: u16) -> TerminalTab {
    let (_, frames) = mpsc::channel::<Frame>();
    let frame = replay(b"one\ntwo", 20, rows);
    let mut tab = TerminalTab::new("t", Some(frame));
    tab.session = Some(Session::over(Box::new(fake.clone()), frames));
    tab
}

#[test]
fn a_paste_into_a_shell_with_bracketed_paste_cannot_submit() {
    let fake = Fake {
        bracketed: true,
        ..Fake::default()
    };
    let mut tab = tab(&fake, 4);
    assert!(tab.paste("rm -rf /\n"));

    let mut expected = PASTE_START.to_vec();
    expected.extend_from_slice(b"rm -rf /\n");
    expected.extend_from_slice(PASTE_END);
    assert_eq!(
        *fake.written.borrow(),
        expected,
        "the newline arrives inside the markers, so the shell inserts it"
    );

    // A payload that carries an end marker of its own cannot close the bracket early: what
    // follows it would arrive as keystrokes, which is the injection 2004 exists to stop.
    fake.written.borrow_mut().clear();
    assert!(tab.paste("a\x1b[201~rm -rf /\n"));
    let mut expected = PASTE_START.to_vec();
    expected.extend_from_slice(b"arm -rf /\n");
    expected.extend_from_slice(PASTE_END);
    assert_eq!(*fake.written.borrow(), expected);
}

#[test]
fn a_paste_into_a_shell_without_it_arrives_unchanged() {
    let fake = Fake::default();
    let mut tab = tab(&fake, 4);
    assert!(tab.paste("echo hi\n"));

    assert_eq!(
        *fake.written.borrow(),
        b"echo hi\n".to_vec(),
        "a shell that never asked for 2004 would print the markers"
    );
}

#[test]
fn a_page_moves_the_grid_a_screen_and_zero_follows_the_bottom_again() {
    let fake = Fake::default();
    let mut tab = tab(&fake, 10);
    assert_eq!(tab.offset(), 0);

    assert!(tab.scroll_page(1));
    assert_eq!(tab.offset(), 9, "a page is one row short of the grid");
    assert!(tab.scroll_page(1));
    assert_eq!(tab.offset(), 18);

    assert!(tab.scroll_page(-1));
    assert_eq!(tab.offset(), 9);

    // Zero is the live bottom: the viewport follows new output from there.
    assert!(tab.scroll_to(0));
    assert_eq!(tab.offset(), 0);
    assert_eq!(*fake.scrolled.borrow(), [9, 18, 9, 0]);

    // A step that would go past the bottom stops at it.
    tab.scroll_lines(-5);
    assert_eq!(tab.offset(), 0);
    assert_eq!(*fake.scrolled.borrow(), [9, 18, 9, 0, 0]);
}

#[test]
fn a_dead_shell_takes_no_paste() {
    let fake = Fake::default();
    let mut tab = tab(&fake, 4);
    tab.ended = Some("the shell exited".to_string());
    assert!(!tab.paste("hello"), "there is nothing to paste into");
    assert!(fake.written.borrow().is_empty());
}

#[test]
fn typing_or_pasting_into_a_view_of_the_history_returns_to_the_bottom() {
    let fake = Fake::default();
    let mut tab = tab(&fake, 8);
    tab.scroll_page(2);
    assert_eq!(tab.offset(), 14);

    let typed = KeyEvent {
        key: Key::Text("x".to_string()),
        mods: Mods::default(),
    };
    assert!(tab.type_key(&typed));
    assert_eq!(
        tab.offset(),
        0,
        "the text lands where the reader can see it"
    );

    // The same rule for a paste: what arrived has to be on screen.
    tab.scroll_page(1);
    assert_eq!(tab.offset(), 7);
    assert!(tab.paste("y"));
    assert_eq!(tab.offset(), 0);
}
