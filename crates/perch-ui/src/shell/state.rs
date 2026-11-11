//! The window's state: the sessions and which one is active, plus what every region reads — the
//! worktree, one transcript, the terminal tabs and the prompt.

use std::cell::Cell;
use std::path::PathBuf;

use perch_files::Workspace;
use perch_store::SessionMeta;
use rune::TextEdit;

use super::answer::Answering;
use super::live::{Idle, Live, Waiting};
use super::pane::Pane;
use super::panel_form::PanelForm;
use super::settings::Panel;
use super::terminal::TerminalTab;
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
    /// The pane the workspace draws: the one value the strip's active tab is derived from.
    pub(super) pane: Pane,
    /// The files the strip carries, in the order they were opened. Their paths are absolute.
    pub(super) files_open: Vec<PathBuf>,
    /// The settings panel, when the pane is the settings: a draft, never the live settings.
    pub(super) settings: Option<Panel>,
    /// The theme the window draws when no panel is open: the saved word, read once when the shell
    /// was built and again when the panel opens or saves. The panel's draft answers while it is
    /// open, so a pick is drawn before Save.
    pub(super) dark: bool,
    /// What the plugin column's controls hold: each input's text and the last refusal.
    pub(super) panel: PanelForm,
    pub(super) folder: super::folder::Folder,
    /// The command list above the composer: whether typing `/` opened it, and the picked row.
    pub(super) menu: super::composer_menu::Menu,
    /// The palette, when the pane is the palette.
    pub(super) palette: Option<super::palette::Palette>,
    /// The file finder, when its overlay is open: the list read on the way in and the rows kept.
    pub(super) finder: Option<super::finder::Finder>,
    /// Whether the dock is hidden: the palette's Toggle dock action flips it.
    pub(super) dock_hidden: bool,
    /// Whether the keyboard is in the session list, where Up and Down move and Escape returns.
    pub(super) sessions_focused: bool,
    /// The session row armed by one press of its delete control, waiting for the second.
    pub(super) pending_delete: Option<usize>,
    /// `/quit` was run: the next frame closes the window.
    pub(super) quit: bool,
    /// `/sessions` asked for the focus: taken once, by the press handler that has a `Tx`.
    pub(super) focus_sessions: bool,
}

impl<L: Live> Shell<L> {
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

    /// Whether the window draws the dark kit.
    ///
    /// The panel's draft answers while it is open, so the theme picked there is the theme drawn
    /// before Save; otherwise it is the saved word, which is read when the shell is built and when
    /// the panel opens or saves — never per frame, because reading it asks the app, and the app
    /// reads `settings.toml` and the keychain.
    pub fn dark(&self) -> bool {
        match self.settings.as_ref() {
            Some(panel) => is_dark(&panel.theme),
            None => self.dark,
        }
    }

    /// The saved theme, drawn from the next frame: the word a restart reads back from disk.
    pub fn set_theme(&mut self, theme: &str) {
        self.dark = is_dark(theme);
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

/// Whether `theme` draws the dark kit.
///
/// Light is the one offered word that is not the default, so every other word — including one a
/// person typed into `settings.toml` by hand — draws the default dark rather than a window with
/// no readable text.
fn is_dark(theme: &str) -> bool {
    theme != "light"
}
