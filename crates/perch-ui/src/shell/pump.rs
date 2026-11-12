//! Folding a frame of the world in: the turn's events, the dock's frames, and the grid the pty is
//! told about.

use std::time::Duration;

use rune::TextStyle;
use rune_kit::Kit;

use super::live::Live;
use super::state::Shell;
use super::terminal::TerminalTab;
use super::ticks::{SHELL_TICK, TURN_TICK};

/// A monospace glyph's advance, in ems. The text system knows the exact width per family, and the
/// dock only needs a size the shell can wrap its prompt at, so this is one number for all of them.
const ADVANCE: f32 = 0.6;

impl<L: Live> Shell<L> {
    /// Folds one frame of the world in: the turn's events, the app's transcript, and every tab's
    /// frames. Returns whether anything the window draws changed.
    ///
    /// `budget` is what the turn may spend waiting for its next event; nothing else here waits,
    /// so a frame is never held longer than that.
    pub fn pump(&mut self, budget: Duration) -> bool {
        let mut changed = false;
        // A listing that arrived while nothing else was moving is still a change the window draws,
        // so it is taken on every frame rather than only when a press happens to ask again.
        changed |= self.absorb_models();
        // A turn that is not in flight has nothing to wait for, so the budget collapses to zero
        // and a frame spent only on the dock never blocks. The app is borrowed for exactly as
        // long as it takes to read it; what it said is then folded in with no borrow alive.
        let budget = if self.live.as_ref().is_some_and(Live::busy) {
            budget
        } else {
            Duration::ZERO
        };
        let pumped = self.live.as_mut().map(|live| {
            let event = live.pump(budget);
            let waiting = live.waiting();
            let transcript = event.then(|| live.transcript().cloned()).flatten();
            (event, waiting, transcript)
        });
        if let Some((event, waiting, transcript)) = pumped {
            changed |= event;
            if self.waiting != waiting {
                // A new question starts with nothing picked, so Enter can never answer the
                // previous one by accident.
                self.clear_answer();
            }
            self.waiting = waiting;
            if let Some(transcript) = transcript {
                self.transcript = transcript;
            }
        }
        let grid = self.grid.get();
        for index in 0..self.terminals.len() {
            let (frame, ended) = self.terminals[index].pump(grid);
            if let Some(frame) = frame {
                self.set_frame(index, frame);
                changed = true;
            }
            changed |= ended;
        }
        changed
    }

    /// How long until the window should look again, when something is still moving.
    ///
    /// `None` is a window with nothing to watch: it then draws only when a key or a press says so.
    pub fn next_wake(&self) -> Option<Duration> {
        // A listing on its way needs frames of its own: waiting for a press is how the picker
        // stays empty after the provider has already answered.
        if self.models_loading() {
            return Some(TURN_TICK);
        }
        if self.live.as_ref().is_some_and(Live::busy) {
            return Some(TURN_TICK);
        }
        if self.terminals.iter().any(TerminalTab::live_shell) {
            return Some(SHELL_TICK);
        }
        None
    }

    /// Measures the dock and remembers the grid the pty should be at. The view calls this with
    /// the rect it drew last frame; it is a measurement, not a mutation of anything shown.
    pub fn measure(&self, width: f32, height: f32, kit: &Kit) {
        let grid = grid_for(width, height, kit.ty.mono);
        if self.grid.get() != grid {
            self.grid.set(grid);
        }
    }

    /// Takes the app's transcript and its waiting state, without pumping.
    pub(super) fn refresh(&mut self) {
        // The provider's model list is fetched on its own thread: a frame takes the answer if it
        // has arrived and never waits for it.
        self.absorb_models();
        let (waiting, transcript) = match self.live.as_ref() {
            Some(live) => (live.waiting(), live.transcript().cloned()),
            None => return,
        };
        if self.waiting != waiting {
            self.clear_answer();
        }
        self.waiting = waiting;
        self.transcript = transcript.unwrap_or_default();
    }
}

/// The grid a pty is told about for a dock of this size, in cells.
///
/// A monospace face advances by about [`ADVANCE`] ems, which is what makes a column count; the
/// row count is the height over the line height the style asks for. The dock draws the same
/// number of rows it tells the pty about, so this is the only place that answers how many lines
/// of text a box of that size holds — two answers would drift and leave a strip under the last.
pub(crate) fn grid_for(width: f32, height: f32, mono: TextStyle) -> (u16, u16) {
    let advance = (mono.size * ADVANCE + mono.tracking).max(1.0);
    let line = if mono.line_height > 0.0 {
        mono.line_height
    } else {
        mono.size * 1.25
    };
    let cols = (width / advance).floor().clamp(20.0, 400.0) as u16;
    let rows = (height / line.max(1.0)).floor().clamp(3.0, 200.0) as u16;
    (cols, rows)
}

#[cfg(test)]
mod tests {
    use super::{ADVANCE, grid_for};
    use rune::TextStyle;

    #[test]
    fn the_dock_grid_follows_the_space_it_drew_in() {
        let mono = TextStyle {
            line_height: 16.0,
            ..TextStyle::new(12.0, 400.0)
        };
        let (cols, rows) = grid_for(12.0 * ADVANCE * 90.0, 160.0, mono);
        assert_eq!((cols, rows), (90, 10));
        // A dock too small for a shell still gets a grid a program can start in.
        assert_eq!(grid_for(1.0, 1.0, mono), (20, 3));
    }
}
