//! The prompt history behind Up and Down: walking back through what this worktree has sent.
//!
//! Only the wiring lives here — the thread's slot, the file it came from, and the key. The walk's
//! order and state machine are [`history_walk`], and the ranking is [`perch_store`]'s.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use perch_store::history::History;
use perch_store::history_path;
use rune::{Key, KeyEvent};

use super::composer_menu::edit;
use super::history_walk::Memo;
use super::live::Live;
use super::state::Shell;
use super::waiting::Waiting;

/// The application id whose data directory holds the history: the one the window's app opens.
const APP_ID: &str = "dev.jyz.perch";
/// The folder under it: one file per worktree.
const DIR: &str = "history";

thread_local! {
    /// What this thread's window is walking: the history of the worktree its composer is in, the
    /// file that history came from, and the step the walk is on. Thread-scoped because a window
    /// draws and reads its keys on one thread.
    static MEMO: RefCell<Memo> = const { RefCell::new(Memo::new()) };
}

/// Applies Up and Down to the prompt: back through what was sent, forward to the draft again.
///
/// Up starts only on an empty box, so a caret key in text the person typed stays the field's.
pub(super) fn history_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.alt || ev.mods.ctrl || ev.mods.shift {
        return false;
    }
    let up = match &ev.key {
        Key::Up => true,
        Key::Down => false,
        _ => return false,
    };
    if shell.waiting() != Waiting::Nothing {
        return false;
    }
    let text = shell.prompt_text().to_owned();
    let step = MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        memo.open(shell, &text);
        memo.step(up, &text)
    });
    let Some(next) = step.text else {
        return step.taken;
    };
    let focused = shell.prompt.focused;
    shell.prompt = edit(&next);
    shell.prompt.focused = focused;
    true
}

/// Records `text` as sent from the worktree the composer is in, and rewrites that worktree's file.
pub(super) fn record<L: Live>(shell: &Shell<L>, text: &str) {
    MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        memo.open(shell, "");
        memo.rewind();
        // The store keeps two prompts inside one clock tick in order, so this passes the clock as it is.
        memo.history.record(text, now());
        if let Some(path) = memo.path.as_deref() {
            // A history that cannot be written costs the next start, never this turn.
            let _ = memo.history.save(path);
        }
    });
}

/// The file this worktree's history is kept in, under the machine's data directory.
///
/// `PERCH_HOME`, when a test or sandbox sets one, stands in for the home directory.
pub fn file(worktree: &Path) -> Option<PathBuf> {
    let home = std::env::var_os("PERCH_HOME");
    let var = |name: &str| match (&home, name) {
        (Some(home), "HOME" | "USERPROFILE") => Some(home.clone()),
        _ => std::env::var_os(name),
    };
    let dirs = rune::dirs::AppDirs::from_env(APP_ID, var)?;
    Some(history_path::path_in(&dirs.data().join(DIR), worktree))
}

/// Loads the history of the worktree now shown, when it is not the one already loaded.
impl Memo {
    fn open<L: Live>(&mut self, shell: &Shell<L>, text: &str) {
        let worktree = shell
            .active_session()
            .map(|session| session.worktree.clone());
        if worktree != self.worktree {
            self.rewind();
            self.path = worktree.as_deref().and_then(file);
            self.history = self
                .path
                .as_deref()
                .map_or_else(History::new, |path| History::load(path, now()));
            self.worktree = worktree;
        }
        self.keep_or_end(text);
    }
}

/// Unix milliseconds now. A history with no usable clock stamps everything at the epoch, which
/// ranks by recency alone rather than refusing to work.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}
