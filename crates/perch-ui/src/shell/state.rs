//! The window's state: the sessions and which one is active, plus what every region reads — the
//! worktree, one transcript, the terminal tabs and the prompt.

use std::cell::Cell;

use perch_files::Workspace;
use perch_store::SessionMeta;
use rune::TextEdit;

use super::answer::Answering;
use super::live::{Idle, Live, Waiting};
use super::panel_form::PanelForm;
use super::settings::Panel;
use super::terminal::{GRID, TerminalTab};
use crate::files::FilePanel;
use crate::transcript::Transcript;

/// The sessions the window shows and which one is active, plus the state every region reads:
/// the worktree, one transcript, the terminal tabs and the prompt.
///
/// A session switch is a state swap: the list, the tabs, the transcript and the terminal all
/// read from this one index, and nothing is reloaded from disk.
pub struct Shell<L: Live = Idle> {
    pub(super) sessions: Vec<SessionMeta>,
    pub(super) active: usize,
    pub(super) workspace: Option<Workspace>,
    pub(super) files: Option<FilePanel>,
    /// The transcript this window draws. With a live app it is the app's transcript, copied on
    /// the drawing thread each time the app says something changed.
    pub(super) transcript: Transcript,
    /// The app behind the window, when there is one.
    pub(super) live: Option<L>,
    /// What the app said it was waiting on at the last pump.
    pub(super) waiting: Waiting,
    /// The answer being given to a waiting question: what is picked, and the text field.
    pub(super) answer: Answering,
    pub(super) terminals: Vec<TerminalTab>,
    pub(super) terminal: usize,
    /// The grid the dock measured last, as the pty is told it.
    pub(super) grid: Cell<(u16, u16)>,
    pub(super) prompt: TextEdit,
    /// The settings panel, when it is open: a draft, never the live settings.
    pub(super) settings: Option<Panel>,
    /// What the plugin column's controls hold: each input's text and the last refusal.
    pub(super) panel: PanelForm,
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
            answer: Answering::default(),
            terminals: Vec::new(),
            terminal: 0,
            grid: Cell::new(GRID),
            prompt: TextEdit::default(),
            settings: None,
            panel: PanelForm::default(),
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
    /// [`paint_rows`](crate::terminal_view::paint_rows) makes of the frame, which is the same
    /// path a live tab takes.
    #[must_use]
    pub fn with_frame(mut self, title: impl Into<String>, frame: perch_term::Frame) -> Shell<L> {
        self.terminals.push(TerminalTab::new(title, Some(frame)));
        self.terminal = self.terminals.len() - 1;
        self
    }

    /// The active session's index.
    pub fn active(&self) -> usize {
        self.active
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
    /// that owns it: the panel is state, and [`files_view`](crate::files::files_view) is pure.
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

    /// Every terminal tab, in strip order.
    pub fn terminals(&self) -> &[TerminalTab] {
        &self.terminals
    }

    /// The terminal tab the dock shows.
    pub fn active_terminal(&self) -> Option<&TerminalTab> {
        self.terminals.get(self.terminal)
    }

    /// The prompt's text as the user left it.
    pub fn prompt_text(&self) -> &str {
        &self.prompt.text
    }
}
