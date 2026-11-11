//! What the dock's pointer and keys do: the wheel and the page keys move the terminal's history,
//! a drag picks cells and copies them, ⌘C copies again, and ⌘V pastes.

use rune::{ClipboardContent, Drag, Key, KeyEvent, Phase, Tx, WheelEvent};

use super::super::live::Live;
use super::super::state::Shell;
use super::super::terminal::cell_at;

/// Sends one press to the shown terminal tab, or does what the dock itself owns. True when the
/// shell was handed it or the grid moved.
pub fn terminal_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let taken = dispatch(shell, ev, tx);
    if taken {
        // A key is direct manipulation, not a transition. The grid lands its own changes
        // (`instant_changes`), but a press also moves what sits outside it — the header's words
        // gain a width when the view pages into the history — so the commit itself is landed.
        tx.instant();
    }
    taken
}

/// What `ev` asks of the dock: the clipboard pair, the page keys, or the shell.
fn dispatch<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // ⌘ chords are the window's own: a terminal never wants ⌘Q as text. The two it does own are
    // the clipboard pair, which is why they are answered here and not by the window's map.
    if ev.mods.cmd {
        return match &ev.key {
            Key::Command('c') => copy(shell, tx),
            Key::Command('v') => {
                // The clipboard is read by the shell, off the event thread; what arrives is
                // pasted with the markers the program asked for, if it asked for any.
                tx.read_clipboard(paste::<L>);
                true
            }
            _ => false,
        };
    }
    // Shift+PageUp and Shift+PageDown move the history rather than reaching the shell: a program
    // that got them would page its own output, which is not what the reader is looking at.
    if ev.mods.shift {
        let pages = match ev.key {
            Key::PageUp => 1,
            Key::PageDown => -1,
            _ => 0,
        };
        if pages != 0 {
            let index = shell.terminal;
            return shell
                .terminals
                .get_mut(index)
                .is_some_and(|tab| tab.scroll_page(pages));
        }
    }
    if ev.mods.alt {
        return false;
    }
    let index = shell.terminal;
    match shell.terminals.get_mut(index) {
        Some(tab) => tab.type_key(ev),
        None => false,
    }
}

/// Copies the shown tab's selection again, for ⌘C after a drag.
fn copy<L: Live>(shell: &mut Shell<L>, tx: &mut Tx) -> bool {
    let index = shell.terminal;
    let Some(tab) = shell.terminals.get_mut(index) else {
        return false;
    };
    tab.copy(tx).is_some()
}

/// Pastes what the clipboard held into the shown tab.
fn paste<L: Live>(shell: &mut Shell<L>, content: &ClipboardContent, _: &mut Tx) {
    let index = shell.terminal;
    if let Some(tab) = shell.terminals.get_mut(index) {
        tab.paste(content.text());
    }
}

/// Moves the grid by the wheel's delta: a terminal scrolls its own history under the pointer, so
/// a started gesture keeps coming here for the rest of its momentum.
pub(super) fn terminal_wheel<L: Live>(
    shell: &mut Shell<L>,
    ev: WheelEvent,
    _: &mut Tx,
    row: f32,
) -> bool {
    let lines = lines_for(ev.dy, row);
    if lines == 0 {
        return false;
    }
    let index = shell.terminal;
    shell
        .terminals
        .get_mut(index)
        .is_some_and(|tab| tab.scroll_lines(lines))
}

/// A drag picks the cells it crossed, and the release copies what they hold. A press that never
/// moved is a click, and a click clears the selection.
pub(super) fn terminal_drag<L: Live>(shell: &mut Shell<L>, drag: &mut Drag, tx: &mut Tx) {
    let index = shell.terminal;
    let Some(tab) = shell.terminals.get_mut(index) else {
        return;
    };
    let Some((cols, rows)) = tab.frame().map(|frame| (frame.cols, frame.height)) else {
        return;
    };
    let (row, col) = cell_at(drag.x, drag.y, drag.w, drag.h, cols, rows);
    match drag.phase {
        Phase::Down => tab.press_cell(row, col),
        Phase::Move => tab.extend_cell(row, col),
        Phase::Up if drag.cancelled() => tab.clear_selection(),
        Phase::Up => {
            tab.extend_cell(row, col);
            tab.release(tx);
        }
    }
}

/// How many rows a wheel delta of `dy` points moves, at least one either way.
///
/// Positive `dy` moves the content down, which shows what is above the bottom: that is one row
/// further into the history, the same direction the page keys go.
fn lines_for(dy: f32, row: f32) -> isize {
    let row = if row > 0.0 { row } else { 17.0 };
    let lines = (dy / row).round();
    if lines == 0.0 {
        return if dy == 0.0 { 0 } else { dy.signum() as isize };
    }
    lines as isize
}

#[cfg(test)]
mod tests {
    use super::lines_for;

    #[test]
    fn a_wheel_delta_is_a_number_of_rows_and_never_a_fraction_of_one() {
        assert_eq!(lines_for(0.0, 17.0), 0, "a still wheel moves nothing");
        assert_eq!(lines_for(34.0, 17.0), 2);
        assert_eq!(
            lines_for(-34.0, 17.0),
            -2,
            "the other way is the live bottom"
        );
        assert_eq!(
            lines_for(4.0, 17.0),
            1,
            "a nudge too small for a row still moves one"
        );
        assert_eq!(lines_for(-0.5, 17.0), -1);
        assert_eq!(lines_for(10.0, 0.0), 1, "no row height falls back to a row");
    }
}
