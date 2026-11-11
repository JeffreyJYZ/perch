//! The shell's constructors: a window with no app, with a worktree, with an app, with a frame.
//!
//! Kept beside the state rather than in it, so the fields and their accessors stay one readable
//! file and every way of building a window reads in one place.

use std::cell::Cell;

use perch_files::Workspace;
use perch_store::SessionMeta;
use rune::TextEdit;

use super::answer::Answering;
use super::folder::Folder;
use super::live::{Idle, Live, Waiting};
use super::pane::Pane;
use super::panel_form::PanelForm;
use super::state::Shell;
use super::terminal::{GRID, TerminalTab};
use crate::files::FilePanel;
use crate::transcript::Transcript;

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
            pane: Pane::Session,
            files_open: Vec::new(),
            settings: None,
            dark: true,
            panel: PanelForm::default(),
            folder: Folder::default(),
            menu: super::composer_menu::Menu::default(),
            palette: None,
            dock_hidden: false,
            sessions_focused: false,
            pending_delete: None,
            quit: false,
            focus_sessions: false,
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
        // The saved theme is read here, once, at the moment the window is built: the app's
        // `settings` reads `settings.toml` and the keychain, so asking it per frame would ask the
        // keychain per frame. The panel opening and a save are the other two reads.
        if let Some(theme) = self.live.as_ref().and_then(Live::settings) {
            self.set_theme(&theme.theme);
        }
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
}
