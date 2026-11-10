//! The window: one left column of sessions over files, the transcript in the centre with the
//! prompt under it, and the terminal dock under the transcript only — so the dock belongs to the
//! session it shows. Every state it shows is a word ("waiting", "approval needed", "the shell
//! exited"); colour never carries meaning on its own.
//!
//! [`Shell`] is state, so a window can be built without a live process behind it: [`Shell::idle`]
//! holds the session list alone, [`Shell::with_workspace`] also reads a worktree into the file
//! panel, [`Shell::with_frame`] seeds a terminal tab, and [`Shell::with_live`] attaches the app
//! that owns the sessions — [`Live`] is the whole of what the window asks of that app.
//!
//! Nothing here appends a user turn: the composer hands its text to [`Live::prompt`], so the
//! column and the log can never disagree about what was asked.

use std::cell::Cell;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use perch_files::Workspace;
use perch_store::SessionMeta;
use perch_term::{
    Color as TermColor, Frame, Key as TermKey, Mods as TermMods, TermHandle, TermSession,
};
use rune::{
    Color, Cx, El, Key, KeyEvent, Mods, ScrollSpec, TextEdit, TextStyle, Tx, col, el, row, text,
};
use rune_kit::Kit;
use rune_kit::TextArea;
use rune_kit::glyphs;
use rune_kit::shell::{SidebarRow, TabItem, TabOrientation, TabStrip};

use crate::files::{FilePanel, files_view, note, section_label};
use crate::terminal_view::{RowRun, paint_rows};
use crate::transcript::{Transcript, transcript_view};

/// The left column's width, in points.
pub const COLUMN: f32 = 276.0;
/// The terminal dock's height, in points.
const DOCK: f32 = 220.0;
/// The tab strip's row height, in points.
const TABS: f32 = 38.0;
/// How tall one session row is, in points.
const ROW: f32 = 40.0;
/// The grid a tab starts at, before the dock has been measured once.
pub const GRID: (u16, u16) = (100, 12);
/// A monospace glyph's advance, in ems. The text system knows the exact width per family, and the
/// dock only needs a size the shell can wrap its prompt at, so this is one number for all of them.
const ADVANCE: f32 = 0.6;
/// What one frame may spend folding a turn's events. The rest of the frame is the frame's: a
/// drawing thread that waits on a turn is a window that stops responding.
pub const BUDGET: Duration = Duration::from_millis(1);
/// How long the window waits before polling again while a turn is streaming.
pub const TURN_TICK: Duration = Duration::from_millis(8);
/// The same for a live shell, which has no channel to wake the window with: it is polled on a
/// tick because its output arrives whether or not anyone asked.
pub const SHELL_TICK: Duration = Duration::from_millis(16);
/// How many frames one tab may take in one pump. A burst of output is one frame plus whatever
/// queued behind it, and a tab that flooded must not spend the whole frame draining.
const FRAMES_PER_PUMP: usize = 8;
/// What the dock says when the shell on the other end of the pty has exited.
pub const EXITED: &str = "the shell exited";
/// What the dock says about a tab whose shell has not printed anything yet.
const WAITING: &str = "waiting";
/// What the dock says about a tab whose shell is still running.
const RUNNING: &str = "running";
/// What the dock says when there is no tab at all.
const STOPPED: &str = "stopped";

/// What the window asks of the app behind it.
///
/// One trait, because there is one app: the shell never reaches past it for state, and a test can
/// stand [`Idle`] in its place. Every method is called on the drawing thread.
pub trait Live: 'static {
    /// Folds what the turn has posted into the app's transcript, waiting at most `budget`, and
    /// reports whether anything changed.
    fn pump(&mut self, budget: Duration) -> bool;

    /// The active session's transcript, when a session is open.
    fn transcript(&self) -> Option<&Transcript>;

    /// What the active session is waiting on.
    fn waiting(&self) -> Waiting;

    /// Whether a turn is in flight, so the window knows to keep polling.
    fn busy(&self) -> bool;

    /// Sends `text` as a turn. The error is why it was not sent, in words.
    fn prompt(&mut self, text: String) -> std::result::Result<(), String>;

    /// Stops the turn: whatever is being waited on is settled first, so no call runs on the
    /// strength of an answer that was never given.
    fn cancel(&mut self);

    /// Allows the call that is waiting.
    fn approve(&mut self);

    /// Refuses it.
    fn deny(&mut self);

    /// Answers the waiting question with the option at `index`, counted from zero.
    fn pick(&mut self, index: usize);

    /// Shows the session at `index`.
    fn switch(&mut self, index: usize);
}

/// A window with no app behind it: the layout, the file panel and the dock alone.
///
/// This is what a `--tree` dump and a layout test run against: no data directory, no keychain.
pub struct Idle;

impl Live for Idle {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Err("no session is open".to_string())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
}

/// What the active session is waiting on, as the key map needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Waiting {
    /// Nothing: a turn is running, or none has started.
    #[default]
    Nothing,
    /// A call that needs the user's decision.
    Approval,
    /// The model's question, with `options` choices.
    Question {
        /// How many options the question offered.
        options: usize,
    },
}

impl Waiting {
    /// The words the window says while this is what it waits on.
    pub fn words(self) -> &'static str {
        match self {
            Waiting::Nothing => "",
            Waiting::Approval => "approval needed",
            Waiting::Question { .. } => "answer needed",
        }
    }
}

/// What one key press asks of the app.
///
/// The map from a press to this is [`intent`], the one place the window's keys are decided; what
/// each one does is [`Shell::apply`], and the app is the only thing that ever sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Send the composer's text as a turn.
    Submit,
    /// Let the field put a line break in the composer.
    Newline,
    /// Stop the turn.
    Cancel,
    /// Allow the call that is waiting.
    Approve,
    /// Refuse it.
    Deny,
    /// Highlight the waiting question's option at this index (zero-based).
    Select(usize),
    /// Answer the question with the option [`Intent::Select`] highlighted.
    Confirm,
    /// Nothing this window does with that key.
    Ignore,
}

/// What `ev` asks for, given what the session is waiting on: Enter sends, Shift+Enter breaks the
/// line, Ctrl+C interrupts, and while an approval or a question is up a number key picks an option,
/// Enter confirms it and Escape refuses or dismisses it.
pub fn intent(ev: &KeyEvent, waiting: Waiting) -> Intent {
    // Ctrl+C is the shell's interrupt, not a character: it stops the turn whatever is on screen.
    // On macOS a Control chord arrives as text with `ctrl` set, so this is where it lands.
    if ev.mods.ctrl
        && !ev.mods.cmd
        && matches!(&ev.key, Key::Text(text) if text.eq_ignore_ascii_case("c"))
    {
        return Intent::Cancel;
    }
    // A chord with Cmd or Option belongs to a shortcut or to the input method, never to a turn.
    if ev.mods.cmd || ev.mods.alt {
        return Intent::Ignore;
    }
    match &ev.key {
        Key::Enter if ev.mods.shift => Intent::Newline,
        Key::Enter => match waiting {
            Waiting::Approval => Intent::Approve,
            Waiting::Question { .. } => Intent::Confirm,
            Waiting::Nothing => Intent::Submit,
        },
        Key::Escape => match waiting {
            Waiting::Approval => Intent::Deny,
            Waiting::Question { .. } => Intent::Cancel,
            Waiting::Nothing => Intent::Ignore,
        },
        Key::Text(text) => match (waiting, number(text)) {
            (Waiting::Question { options }, Some(index)) if index < options => {
                Intent::Select(index)
            }
            _ => Intent::Ignore,
        },
        _ => Intent::Ignore,
    }
}

/// The zero-based option a digit key names, when it names one.
fn number(text: &str) -> Option<usize> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    first
        .to_digit(10)
        .filter(|digit| (1..=9).contains(digit))
        .map(|digit| digit as usize - 1)
}

/// One terminal tab: its title, the last frame its thread published, and the shell behind it.
///
/// The frame is the whole of what the dock paints, because the terminal's emulator lives on its
/// own thread and only plain snapshots cross to the UI.
pub struct TerminalTab {
    title: String,
    frame: Option<Frame>,
    /// The pty this tab drives, and the channel its frames arrive on. Seeded tabs have none.
    session: Option<TermSlot>,
    /// Why the shell stopped, in words, once it has. A dead shell says so instead of leaving the
    /// last frame on screen as if it were still printing.
    ended: Option<String>,
    /// The grid the pty was last told about.
    grid: (u16, u16),
}

/// The process side of a tab: the handle the UI drives and the frames coming back.
struct TermSlot {
    handle: TermHandle,
    frames: Receiver<Frame>,
}

impl TerminalTab {
    /// A tab called `title` showing `frame`, or an empty grid while its thread has published
    /// nothing yet.
    pub fn new(title: impl Into<String>, frame: Option<Frame>) -> TerminalTab {
        TerminalTab {
            title: title.into(),
            frame,
            session: None,
            ended: None,
            grid: GRID,
        }
    }

    /// A tab over a spawned shell.
    fn live(
        title: impl Into<String>,
        handle: TermHandle,
        frames: Receiver<Frame>,
        grid: (u16, u16),
    ) -> TerminalTab {
        TerminalTab {
            title: title.into(),
            frame: None,
            session: Some(TermSlot { handle, frames }),
            ended: None,
            grid,
        }
    }

    /// A tab whose shell could not be started, saying why instead of showing an empty grid.
    fn failed(title: impl Into<String>, words: impl Into<String>) -> TerminalTab {
        TerminalTab {
            title: title.into(),
            frame: None,
            session: None,
            ended: Some(words.into()),
            grid: GRID,
        }
    }

    /// The tab's title, as the dock shows it.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The last frame the tab's thread published.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// Why the shell stopped, when it has.
    pub fn ended(&self) -> Option<&str> {
        self.ended.as_deref()
    }

    /// The grid the pty behind this tab was last told about.
    pub fn grid(&self) -> (u16, u16) {
        self.grid
    }

    /// The word the dock shows for this tab.
    pub fn state(&self) -> &'static str {
        if self.ended.is_some() {
            return "exited";
        }
        if self.frame.is_some() {
            RUNNING
        } else {
            WAITING
        }
    }

    /// Whether a shell is still running behind this tab.
    fn live_shell(&self) -> bool {
        self.session.is_some() && self.ended.is_none()
    }

    /// Takes the newest frames the tab's thread published, tells the pty about a new grid, and
    /// notices a shell that has exited. Never waits: a tab with nothing new costs one `try_recv`.
    ///
    /// Returns the newest frame, which the shell installs through [`Shell::set_frame`], and
    /// whether the shell has just been found dead.
    fn pump(&mut self, grid: (u16, u16)) -> (Option<Frame>, bool) {
        let Some(slot) = self.session.as_mut() else {
            return (None, false);
        };
        let mut newest = None;
        for _ in 0..FRAMES_PER_PUMP {
            match slot.frames.try_recv() {
                Ok(frame) => newest = Some(frame),
                Err(_) => break,
            }
        }
        if self.grid != grid {
            slot.handle.resize(grid.0, grid.1);
            self.grid = grid;
        }
        let ended = self.ended.is_none() && slot.handle.exited();
        if ended {
            self.ended = Some(EXITED.to_string());
        }
        (newest, ended)
    }

    /// Turns one key press into the bytes the shell expects.
    ///
    /// Printable text goes through as the bytes it is; everything a terminal mode can change —
    /// Return, the arrows, Delete, the function keys — is encoded by the terminal's own encoder,
    /// which is the only thing that knows whether the program asked for application cursor keys.
    fn type_key(&mut self, ev: &KeyEvent) -> bool {
        if self.ended.is_some() {
            return false;
        }
        let Some(slot) = self.session.as_mut() else {
            return false;
        };
        match &ev.key {
            Key::Modifier { .. } => false,
            Key::Text(text) => {
                if ev.mods.ctrl {
                    for character in text.chars() {
                        slot.handle.write(&[control(character)]);
                    }
                } else if ev.mods.alt {
                    // Option is Meta: the same escape prefix a real terminal sends.
                    slot.handle.write(b"\x1b");
                    slot.handle.write(text.as_bytes());
                } else {
                    slot.handle.write(text.as_bytes());
                }
                true
            }
            key => match term_event(key, ev.mods) {
                Some(event) => {
                    slot.handle.write(&slot.handle.key(event));
                    true
                }
                None => false,
            },
        }
    }
}

/// `Ctrl` on a US layout clears the top three bits of a letter's ASCII code.
fn control(character: char) -> u8 {
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
fn term_event(key: &Key, mods: Mods) -> Option<perch_term::KeyEvent> {
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

/// The sessions the window shows and which one is active, plus the state every region reads:
/// the worktree, one transcript, the terminal tabs and the prompt.
///
/// A session switch is a state swap: the list, the tabs, the transcript and the terminal all
/// read from this one index, and nothing is reloaded from disk.
pub struct Shell<L: Live = Idle> {
    sessions: Vec<SessionMeta>,
    active: usize,
    workspace: Option<Workspace>,
    files: Option<FilePanel>,
    /// The transcript this window draws. With a live app it is the app's transcript, copied on
    /// the drawing thread each time the app says something changed.
    transcript: Transcript,
    /// The app behind the window, when there is one.
    live: Option<L>,
    /// What the app said it was waiting on at the last pump.
    waiting: Waiting,
    /// The option a number key highlighted while a question waits; Enter answers with it.
    picked: Option<usize>,
    terminals: Vec<TerminalTab>,
    terminal: usize,
    /// The grid the dock measured last, as the pty is told it.
    grid: Cell<(u16, u16)>,
    prompt: TextEdit,
}

impl Shell<Idle> {
    /// A window with no app behind it: the layout, the file panel and the dock alone.
    ///
    /// This is what a layout test and a `--tree` dump build, so neither needs a data directory,
    /// a keychain or a provider.
    pub fn idle(sessions: Vec<SessionMeta>, active: usize) -> Shell<Idle> {
        Shell::new(sessions, active)
    }
}

impl<L: Live> Shell<L> {
    /// The shell over `sessions`, with `active` selected. An out-of-range `active` falls back to
    /// the first session, so a caller never has to clamp it.
    pub fn new(sessions: Vec<SessionMeta>, active: usize) -> Shell<L> {
        let active = if sessions.is_empty() {
            0
        } else {
            active.min(sessions.len() - 1)
        };
        Shell {
            sessions,
            active,
            workspace: None,
            files: None,
            transcript: Transcript::default(),
            live: None,
            waiting: Waiting::Nothing,
            picked: None,
            terminals: Vec::new(),
            terminal: 0,
            grid: Cell::new(GRID),
            prompt: TextEdit::default(),
        }
    }

    /// The shell bound to `workspace`: the file panel is read from it once, so the window shows
    /// the worktree the session belongs to.
    ///
    /// This is the constructor the app uses; passing the workspace in rather than a path keeps a
    /// test's temp repo and the app on the same path, with no second walk.
    pub fn with_workspace(
        sessions: Vec<SessionMeta>,
        active: usize,
        workspace: Workspace,
    ) -> Shell<L> {
        let mut shell = Shell::new(sessions, active);
        shell.files = Some(FilePanel::new(&workspace));
        shell.workspace = Some(workspace);
        shell
    }

    /// The shell plus the app that owns its sessions.
    ///
    /// The type carries which app, so a test can reach the app it attached and assert on what it
    /// did; the window itself only ever goes through [`Live`].
    #[must_use]
    pub fn with_live(mut self, live: L) -> Shell<L> {
        self.live = Some(live);
        self.refresh();
        self
    }

    /// The shell plus a terminal tab showing `frame`.
    ///
    /// A seeded frame is how a test drives the dock without a PTY: the tab draws exactly what
    /// [`paint_rows`] makes of the frame, which is the same path a live tab takes.
    #[must_use]
    pub fn with_frame(mut self, title: impl Into<String>, frame: Frame) -> Shell<L> {
        self.terminals.push(TerminalTab::new(title, Some(frame)));
        self.terminal = self.terminals.len() - 1;
        self
    }

    /// The active session's index.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Selects the session at `index`. An index past the end is ignored rather than clamped: it
    /// names a session that is gone, and switching to a neighbour would switch to the wrong one.
    pub fn select(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active = index;
            if let Some(live) = self.live.as_mut() {
                live.switch(index);
            }
            self.refresh();
        }
    }

    /// Every session's title, in tab order.
    pub fn tab_titles(&self) -> Vec<String> {
        self.sessions
            .iter()
            .map(|session| session.title.clone())
            .collect()
    }

    /// The active session, when there is one.
    pub fn active_session(&self) -> Option<&SessionMeta> {
        self.sessions.get(self.active)
    }

    /// The worktree this shell is bound to, when it was opened with one.
    pub fn workspace(&self) -> Option<&Workspace> {
        self.workspace.as_ref()
    }

    /// The file panel, when the shell was opened with a worktree. Presses belong to the caller
    /// that owns it: the panel is state, and [`files_view`] is pure.
    pub fn files(&self) -> Option<&FilePanel> {
        self.files.as_ref()
    }

    /// The file panel, for the caller attaching the tree's presses.
    pub fn files_mut(&mut self) -> Option<&mut FilePanel> {
        self.files.as_mut()
    }

    /// This session's transcript: the agent's events in, items out.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// The transcript, for a caller folding events in without an app behind the window.
    pub fn transcript_mut(&mut self) -> &mut Transcript {
        &mut self.transcript
    }

    /// The app behind the window, when there is one.
    pub fn live(&self) -> Option<&L> {
        self.live.as_ref()
    }

    /// The app, for a caller driving it directly.
    pub fn live_mut(&mut self) -> Option<&mut L> {
        self.live.as_mut()
    }

    /// What the app said it was waiting on at the last pump.
    pub fn waiting(&self) -> Waiting {
        self.waiting
    }

    /// The option a number key highlighted, while a question waits.
    pub fn picked(&self) -> Option<usize> {
        self.picked
    }

    /// Every terminal tab, in strip order.
    pub fn terminals(&self) -> &[TerminalTab] {
        &self.terminals
    }

    /// The terminal tab the dock shows.
    pub fn active_terminal(&self) -> Option<&TerminalTab> {
        self.terminals.get(self.terminal)
    }

    /// Replaces tab `index`'s frame with the newest one its thread published. An index past the
    /// end is ignored: a frame for a closed tab is not a reason to open one.
    pub fn set_frame(&mut self, index: usize, frame: Frame) {
        if let Some(tab) = self.terminals.get_mut(index) {
            tab.frame = Some(frame);
        }
    }

    /// Shows the terminal tab at `index`. An index past the end is ignored, for the same reason a
    /// session index is: it names a tab that is gone.
    pub fn select_terminal(&mut self, index: usize) {
        if index < self.terminals.len() {
            self.terminal = index;
        }
    }

    /// The prompt's text as the user left it.
    pub fn prompt_text(&self) -> &str {
        &self.prompt.text
    }

    /// Puts `text` in the composer, as typing would.
    pub fn set_prompt(&mut self, text: impl Into<String>) {
        self.prompt = TextEdit::default();
        self.prompt.text = text.into();
    }

    /// Spawns a shell for a new terminal tab, in the active session's worktree, and shows it.
    ///
    /// This blocks until the emulator exists, so it is called when a tab is opened and never from
    /// a frame.
    pub fn open_terminal(&mut self) -> std::result::Result<usize, String> {
        let Some(session) = self.sessions.get(self.active) else {
            return Err("no session to open a shell for".to_string());
        };
        let cwd = session.worktree.clone();
        let title = cwd.file_name().map_or_else(
            || cwd.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let (cols, rows) = self.grid.get();
        let spawned = TermSession::spawn(&cwd, cols, rows);
        let (handle, frames) = match spawned {
            Ok(spawned) => spawned,
            Err(error) => {
                // The dock says why there is no shell, in words, rather than showing an empty
                // grid as if one were starting.
                let words = format!("no shell: {error}");
                self.terminals
                    .push(TerminalTab::failed(title, words.clone()));
                self.terminal = self.terminals.len() - 1;
                return Err(words);
            }
        };
        self.terminals
            .push(TerminalTab::live(title, handle, frames, (cols, rows)));
        self.terminal = self.terminals.len() - 1;
        Ok(self.terminal)
    }

    /// Folds one frame of the world in: the turn's events, the app's transcript, and every tab's
    /// frames. Returns whether anything the window draws changed.
    ///
    /// `budget` is what the turn may spend waiting for its next event; nothing else here waits,
    /// so a frame is never held longer than that.
    pub fn pump(&mut self, budget: Duration) -> bool {
        let mut changed = false;
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
                self.picked = None;
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
        if self.live.as_ref().is_some_and(Live::busy) {
            return Some(TURN_TICK);
        }
        if self.terminals.iter().any(TerminalTab::live_shell) {
            return Some(SHELL_TICK);
        }
        None
    }

    /// Sends the composer's text as a turn, and empties the box when it was sent.
    ///
    /// The app owns the transcript: a refusal is a turn the app refused, and the sentence saying
    /// why is in the transcript the app keeps, not one invented here.
    pub fn submit(&mut self) -> bool {
        let text = self.prompt.text.trim().to_owned();
        if text.is_empty() {
            return false;
        }
        let sent = match self.live.as_mut() {
            Some(live) => live.prompt(text),
            None => return false,
        };
        self.refresh();
        if sent.is_ok() {
            self.prompt = TextEdit::default();
        }
        true
    }

    /// Stops the turn, settling whatever it is waiting on as a refusal first.
    pub fn interrupt(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.cancel();
        }
        self.refresh();
    }

    /// Allows the call that is waiting.
    pub fn approve(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.approve();
        }
        self.refresh();
    }

    /// Refuses the call that is waiting.
    pub fn deny(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.deny();
        }
        self.refresh();
    }

    /// Highlights the waiting question's option at `index`.
    pub fn select_option(&mut self, index: usize) -> bool {
        match self.waiting {
            Waiting::Question { options } if index < options => {
                self.picked = Some(index);
                true
            }
            _ => false,
        }
    }

    /// Answers the waiting question with the highlighted option. Enter with nothing highlighted
    /// answers nothing: a confirmation nobody chose is not an answer.
    pub fn confirm(&mut self) -> bool {
        let Some(index) = self
            .picked
            .filter(|_| matches!(self.waiting, Waiting::Question { .. }))
        else {
            return false;
        };
        self.picked = None;
        if let Some(live) = self.live.as_mut() {
            live.pick(index);
        }
        self.refresh();
        true
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
    fn refresh(&mut self) {
        let (waiting, transcript) = match self.live.as_ref() {
            Some(live) => (live.waiting(), live.transcript().cloned()),
            None => return,
        };
        if self.waiting != waiting {
            self.picked = None;
        }
        self.waiting = waiting;
        self.transcript = transcript.unwrap_or_default();
    }
}

/// The grid a pty is told about for a dock of this size, in cells.
///
/// A monospace face advances by about [`ADVANCE`] ems, which is what makes a column count; the
/// row count is the height over the line height the style asks for.
fn grid_for(width: f32, height: f32, mono: TextStyle) -> (u16, u16) {
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

/// Applies one press to the shell through the app behind it. True when the key was consumed.
///
/// This is the one handler the composer and the window share: a field's own Return would break
/// the line, so the window answers Return first and lets Shift+Return through to the field.
pub fn composer_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx) -> bool {
    match intent(ev, shell.waiting) {
        // A line break is the field's to insert, and nothing else does it for us.
        Intent::Ignore | Intent::Newline => false,
        Intent::Submit => {
            shell.submit();
            true
        }
        Intent::Cancel => {
            shell.interrupt();
            true
        }
        Intent::Approve => {
            shell.approve();
            true
        }
        Intent::Deny => {
            shell.deny();
            true
        }
        Intent::Select(index) => shell.select_option(index),
        // Enter is consumed either way: an unanswered confirmation must not type a newline.
        Intent::Confirm => {
            shell.confirm();
            true
        }
    }
}

/// Sends one press to the shown terminal tab. True when the shell was handed it.
pub fn terminal_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx) -> bool {
    // Cmd chords are the window's own: a terminal never wants ⌘Q as text.
    if ev.mods.cmd {
        return false;
    }
    match shell.terminals.get_mut(shell.terminal) {
        Some(tab) => tab.type_key(ev),
        None => false,
    }
}

/// The whole window.
pub fn view<L: Live>(shell: &Shell<L>, cx: &mut Cx<Shell<L>>) -> El {
    let kit = Kit::neutral(cx.system_dark());
    row()
        .key("perch")
        .w_full()
        .h_full()
        .fill(kit.c.bg_deep)
        .child(column(shell, &kit))
        .child(rule(&kit, "column-rule").w(1.0).h_full())
        .child(workspace(shell, &kit, cx))
}

/// A hairline that separates two regions; the caller gives it its one-pixel axis. Decorative: the
/// regions already name themselves.
fn rule(kit: &Kit, key: &str) -> El {
    el().key(key).fill(kit.c.border_base).a11y_hidden()
}

/// The left column, full window height: the session list above the file tree.
fn column<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    col()
        .key("left-column")
        .role("region")
        .label("Sidebar")
        .w(COLUMN)
        .h_full()
        .min_h(0.0)
        .fill(kit.c.bg_base)
        .child(sessions(shell, kit))
        .child(files(shell, kit))
}

/// The session list, and the tab strip it is the long form of.
fn sessions<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let mut list = col()
        .key("sessions")
        .role("region")
        .label("Sessions")
        .w_full()
        .shrink()
        .px(10.0)
        .pt(12.0)
        .pb(8.0)
        .gap(2.0)
        .child(section_label(kit, "Sessions", "sessions-label"));
    if shell.sessions.is_empty() {
        list = list.child(note(kit, "No sessions yet"));
    }
    for (index, session) in shell.sessions.iter().enumerate() {
        list = list.child(
            SidebarRow::new(session.title.clone())
                .height(ROW)
                .subtitle(describe(session))
                .selected(index == shell.active)
                .on_select(move |s: &mut Shell<L>, _: &mut Tx| s.select(index))
                .build(kit)
                .key(format!("session-{index}")),
        );
    }
    list
}

/// The file tree's slot: the worktree's rows when there is a workspace, and words when there is
/// not, so the region is never blank.
fn files<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    match shell.files.as_ref() {
        Some(panel) => files_view(panel),
        None => col()
            .key("files")
            .role("region")
            .label("Files")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .px(10.0)
            .pt(10.0)
            .pb(10.0)
            .gap(4.0)
            .child(section_label(kit, "Files", "files-label"))
            .child(note(kit, "not loaded")),
    }
}

/// The centre: the tab strip, the transcript with the prompt under it, and the dock under the
/// transcript alone.
fn workspace<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &mut Cx<Shell<L>>) -> El {
    col()
        .key("workspace")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(kit.c.bg_deep)
        .child(tabs(shell, kit))
        .child(centre(shell, kit))
        .child(rule(kit, "dock-rule").w_full().h(1.0))
        .child(dock(shell, kit, cx))
}

/// The session tabs. Selecting one is the same state swap the rows make.
///
/// The strip grows along its parent's main axis, which is right in a titlebar and wrong in a
/// column, so it gets a row of its own: one tab row's height, the transcript takes the rest.
fn tabs<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let items: Vec<TabItem<'_, Shell<L>>> = shell
        .sessions
        .iter()
        .map(|session| {
            TabItem::new(session.id.clone(), session.title.clone())
                .icon(&glyphs::CURSOR_TEXT)
                .closable(false)
        })
        .collect();
    let active = shell.active_session().map(|session| session.id.as_str());
    let strip = TabStrip::new("session-tabs", TabOrientation::Horizontal)
        .label("Session tabs")
        .tabs(items)
        .active(active)
        .on_select(|shell: &mut Shell<L>, id: &str, _: &mut Tx| {
            if let Some(index) = shell.sessions.iter().position(|session| session.id == id) {
                shell.select(index);
            }
        })
        .build(kit);
    row().key("tab-row").w_full().h(TABS).child(strip)
}

/// The transcript column: the turns, and the prompt at the bottom of it.
///
/// The prompt sits in this column rather than under the dock, so the dock's bottom edge is the
/// window's and the composer belongs to what it asks.
fn centre<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    col()
        .key("transcript-column")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(transcript_view(&shell.transcript))
        .child(composer(shell, kit))
}

/// The prompt: one multi-line field, in its own labelled section.
fn composer<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    col()
        .key("composer")
        .w_full()
        .px(20.0)
        .pt(4.0)
        .pb(12.0)
        .gap(4.0)
        .child(section_label(kit, "Prompt", "prompt-label"))
        .child(
            TextArea::new(&shell.prompt)
                .label("Prompt")
                .placeholder("Ask perch…")
                .rows(3)
                .width(None)
                .on_edit(|shell: &mut Shell<L>| &mut shell.prompt)
                .on_key(composer_key::<L>)
                .on_submit(submit::<L>)
                .build(kit),
        )
}

/// Sends what the box holds, as the same path Enter takes.
fn submit<L: Live>(shell: &mut Shell<L>, _: &mut Tx) {
    shell.submit();
}

/// The terminal dock, under the transcript and no wider than it.
fn dock<L: Live>(shell: &Shell<L>, kit: &Kit, cx: &mut Cx<Shell<L>>) -> El {
    // The grid the pty is told about follows what the dock drew last frame, so a resized window
    // reaches the shell's own line wrapping.
    if let Some([width, height]) = cx.layout_size("terminal-rows") {
        shell.measure(width, height, kit);
    }
    let tab = shell.active_terminal();
    let (title, state) = match tab {
        Some(tab) => (tab.title().to_owned(), tab.state()),
        None => (String::new(), STOPPED),
    };
    let status = if title.is_empty() {
        state.to_owned()
    } else {
        format!("{title} · {state}")
    };
    let mut panel = col()
        .key("terminal")
        .role("region")
        .label("Terminal")
        .w_full()
        .h(DOCK)
        .fill(kit.c.bg_base)
        .px(20.0)
        .py(12.0)
        .gap(6.0)
        .child(
            row()
                .w_full()
                .justify_between()
                .child(section_label(kit, "Terminal", "terminal-label"))
                .child(text(status).font(kit.ty.small).color(kit.c.text_faint)),
        );
    // One tab needs no strip: the header already names it. Two or more need one to say which is
    // shown, and to be the thing that switches them.
    if shell.terminals.len() > 1 {
        panel = panel.child(terminal_tabs(shell, kit));
    }
    panel.child(dock_body::<L>(tab, kit))
}

/// The terminal tabs, in strip order, with the shown one selected.
fn terminal_tabs<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let items: Vec<TabItem<'_, Shell<L>>> = shell
        .terminals
        .iter()
        .enumerate()
        .map(|(index, tab)| TabItem::new(index.to_string(), tab.title().to_owned()).closable(false))
        .collect();
    let active = shell.terminal.to_string();
    let strip = TabStrip::new("terminal-tabs", TabOrientation::Horizontal)
        .label("Terminal tabs")
        .tabs(items)
        .active(Some(active.as_str()))
        .on_select(|shell: &mut Shell<L>, id: &str, _: &mut Tx| {
            if let Ok(index) = id.parse::<usize>() {
                shell.select_terminal(index);
            }
        })
        .build(kit);
    row()
        .key("terminal-tab-row")
        .w_full()
        .h(TABS - 6.0)
        .child(strip)
}

/// The dock's grid: the frame's rows as runs, or the reason there is nothing to draw.
fn dock_body<L: Live>(tab: Option<&TerminalTab>, kit: &Kit) -> El {
    // A shell that has exited says so. Its last frame is not painted: a frozen grid reads as a
    // shell that is still printing, and the one thing the window must not do is pretend.
    if let Some(ended) = tab.and_then(TerminalTab::ended) {
        return col()
            .key("terminal-rows")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(note(kit, ended));
    }
    let Some(frame) = tab.and_then(TerminalTab::frame) else {
        return col()
            .key("terminal-rows")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(note(kit, "no shell running"));
    };
    // Follow the bottom, the way a terminal does: the newest output is the row in view. The grid
    // takes keys, so a click into it types at the shell.
    let mut grid = col()
        .key("terminal-rows")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(0.0)
        .focusable(true)
        .on_key(terminal_key::<L>)
        .scroll(ScrollSpec { stick_bottom: true });
    // [`paint_rows`] emits runs top to bottom, so a row is one stretch of entries that name the
    // same `row`; each stretch becomes one line of the grid.
    let mut row: Option<u16> = None;
    let mut group: Vec<RowRun> = Vec::new();
    for run in paint_rows(frame) {
        if row != Some(run.row) {
            if let Some(index) = row {
                grid = grid.child(terminal_row(index, &group, kit));
            }
            group.clear();
            row = Some(run.row);
        }
        group.push(run);
    }
    if let Some(index) = row {
        grid = grid.child(terminal_row(index, &group, kit));
    }
    grid
}

/// One row of the grid: its runs side by side, keyed by the row index so a repaint keeps the
/// lines above it.
fn terminal_row(index: u16, runs: &[RowRun], kit: &Kit) -> El {
    let mut line = row().key(format!("terminal-row-{index}")).w_full().gap(0.0);
    for run in runs {
        line = line.child(terminal_text(run, kit));
    }
    line
}

/// One run of one row: its text, in the weight and slant the cell had.
///
/// A run is contiguous by construction, so a row's runs can be laid out as siblings and still
/// read as the line the shell wrote.
fn terminal_text(run: &RowRun, kit: &Kit) -> El {
    text(run.text.clone())
        .font(TextStyle {
            weight: if run.bold { 600.0 } else { kit.ty.mono.weight },
            italic: run.italic,
            ..kit.ty.mono
        })
        .color(ink(run.fg, kit))
}

/// A cell colour as the painter draws it; the frame's default is the kit's body ink.
fn ink(color: TermColor, kit: &Kit) -> Color {
    match color {
        TermColor::Default => kit.c.text_base,
        TermColor::Rgb(r, g, b) => rgb(r, g, b),
        TermColor::Indexed(index) => indexed(index),
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: 1.0,
    }
}

/// A 256-colour palette entry, as xterm defines it: sixteen named colours, a 6×6×6 cube, then
/// twenty-four greys. The frame carries the palette index, so the table lives at the painter.
fn indexed(index: u8) -> Color {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0..=15 => {
            let (r, g, b) = BASE[usize::from(index)];
            rgb(r, g, b)
        }
        16..=231 => {
            let cube = index - 16;
            rgb(
                LEVELS[usize::from(cube / 36)],
                LEVELS[usize::from((cube % 36) / 6)],
                LEVELS[usize::from(cube % 6)],
            )
        }
        _ => {
            let grey = 8 + (index - 232) * 10;
            rgb(grey, grey, grey)
        }
    }
}

/// A session row's second line: its branch and its worktree, both as words.
fn describe(session: &SessionMeta) -> String {
    let branch = if session.branch.is_empty() {
        "no branch".to_string()
    } else {
        session.branch.clone()
    };
    let worktree = session.worktree.file_name().map_or_else(
        || session.worktree.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!("{branch} · {worktree}")
}

#[cfg(test)]
mod tests {
    use super::{ADVANCE, Intent, Waiting, grid_for, intent};
    use rune::{Key, KeyEvent, Mods, TextStyle};

    fn press(key: Key, shift: bool) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods {
                shift,
                ..Mods::default()
            },
        }
    }

    fn text(value: &str) -> KeyEvent {
        KeyEvent {
            key: Key::Text(value.to_string()),
            mods: Mods::default(),
        }
    }

    const QUESTION: Waiting = Waiting::Question { options: 3 };

    #[test]
    fn enter_sends_and_shift_enter_breaks_the_line() {
        assert_eq!(
            intent(&press(Key::Enter, false), Waiting::Nothing),
            Intent::Submit
        );
        assert_eq!(
            intent(&press(Key::Enter, true), Waiting::Nothing),
            Intent::Newline
        );
        // A turn that is already running takes the same pair: send, or break the line.
        assert_eq!(
            intent(&press(Key::Enter, false), Waiting::Approval),
            Intent::Approve
        );
    }

    #[test]
    fn a_number_key_picks_and_enter_confirms() {
        assert_eq!(intent(&text("2"), QUESTION), Intent::Select(1));
        assert_eq!(
            intent(&text("9"), QUESTION),
            Intent::Ignore,
            "past the last option"
        );
        assert_eq!(
            intent(&text("0"), QUESTION),
            Intent::Ignore,
            "options count from one"
        );
        assert_eq!(intent(&press(Key::Enter, false), QUESTION), Intent::Confirm);
        // With nothing waiting, a digit is typing: the field keeps it.
        assert_eq!(intent(&text("2"), Waiting::Nothing), Intent::Ignore);
    }

    #[test]
    fn escape_refuses_an_approval_and_dismisses_a_question() {
        assert_eq!(
            intent(&press(Key::Escape, false), Waiting::Approval),
            Intent::Deny
        );
        assert_eq!(intent(&press(Key::Escape, false), QUESTION), Intent::Cancel);
        assert_eq!(
            intent(&press(Key::Escape, false), Waiting::Nothing),
            Intent::Ignore
        );
    }

    #[test]
    fn ctrl_c_interrupts_whatever_is_on_screen() {
        let interrupt = KeyEvent {
            key: Key::Text("c".to_string()),
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
        };
        for waiting in [Waiting::Nothing, Waiting::Approval, QUESTION] {
            assert_eq!(intent(&interrupt, waiting), Intent::Cancel, "{waiting:?}");
        }
        // ⌘C is a copy, not an interrupt.
        let copy = KeyEvent {
            key: Key::Command('c'),
            mods: Mods {
                cmd: true,
                ..Mods::default()
            },
        };
        assert_eq!(intent(&copy, Waiting::Nothing), Intent::Ignore);
    }

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
