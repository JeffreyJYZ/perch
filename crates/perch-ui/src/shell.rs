//! The window: one left column of sessions over files, the transcript in the centre with the
//! prompt under it, and the terminal dock under the transcript only — so the dock belongs to the
//! session it shows.
//!
//! Every state the layout shows is a word ("waiting", "done", "failed", "approval needed");
//! colour never carries meaning on its own.
//!
//! [`Shell`] is plain state, so a window can be built without a live process behind it:
//! [`Shell::new`] holds the session list alone, [`Shell::with_workspace`] also reads a worktree
//! into the file panel, and [`Shell::with_frame`] seeds a terminal tab with a frame its thread
//! would otherwise have published. The window itself, [`view`], only reads that state.
//!
//! The prompt is the shell's own: submitting it records the question as a user turn and clears
//! the box. Sending it to a provider belongs to the agent that owns the turn, which reads the
//! transcript's last user item rather than a copy kept here.

use perch_files::Workspace;
use perch_store::SessionMeta;
use perch_term::{Color as TermColor, Frame};
use rune::{Color, Cx, El, ScrollSpec, TextEdit, TextStyle, Tx, col, el, row, text};
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

/// One terminal tab: its title, and the last frame its thread published.
///
/// The frame is the whole of what the dock paints, because the terminal's emulator lives on its
/// own thread and only plain snapshots cross to the UI.
pub struct TerminalTab {
    title: String,
    frame: Option<Frame>,
}

impl TerminalTab {
    /// A tab called `title` showing `frame`, or an empty grid while its thread has published
    /// nothing yet.
    pub fn new(title: impl Into<String>, frame: Option<Frame>) -> TerminalTab {
        TerminalTab {
            title: title.into(),
            frame,
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
}

/// The sessions the window shows and which one is active, plus the state every region reads:
/// the worktree, one transcript, the terminal tabs and the prompt.
///
/// A session switch is a state swap: the list, the tabs, the transcript and the terminal all
/// read from this one index, and nothing is reloaded from disk.
pub struct Shell {
    sessions: Vec<SessionMeta>,
    active: usize,
    workspace: Option<Workspace>,
    files: Option<FilePanel>,
    transcript: Transcript,
    terminals: Vec<TerminalTab>,
    terminal: usize,
    prompt: TextEdit,
}

impl Shell {
    /// The shell over `sessions`, with `active` selected. An out-of-range `active` falls back to
    /// the first session, so a caller never has to clamp it.
    pub fn new(sessions: Vec<SessionMeta>, active: usize) -> Shell {
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
            terminals: Vec::new(),
            terminal: 0,
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
    ) -> Shell {
        let mut shell = Shell::new(sessions, active);
        shell.files = Some(FilePanel::new(&workspace));
        shell.workspace = Some(workspace);
        shell
    }

    /// The shell plus a terminal tab showing `frame`.
    ///
    /// A seeded frame is how a test drives the dock without a PTY: the tab draws exactly what
    /// [`paint_rows`] makes of the frame, which is the same path a live tab takes.
    #[must_use]
    pub fn with_frame(mut self, title: impl Into<String>, frame: Frame) -> Shell {
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

    /// The transcript, for the agent lane folding its events in.
    pub fn transcript_mut(&mut self) -> &mut Transcript {
        &mut self.transcript
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
}

/// The whole window.
pub fn view(shell: &Shell, cx: &mut Cx<Shell>) -> El {
    let kit = Kit::neutral(cx.system_dark());
    row()
        .key("perch")
        .w_full()
        .h_full()
        .fill(kit.c.bg_deep)
        .child(column(shell, &kit))
        .child(rule(&kit, "column-rule").w(1.0).h_full())
        .child(workspace(shell, &kit))
}

/// A hairline that separates two regions; the caller gives it its one-pixel axis. Decorative: the
/// regions already name themselves.
fn rule(kit: &Kit, key: &str) -> El {
    el().key(key).fill(kit.c.border_base).a11y_hidden()
}

/// The left column, full window height: the session list above the file tree.
fn column(shell: &Shell, kit: &Kit) -> El {
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
fn sessions(shell: &Shell, kit: &Kit) -> El {
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
                .on_select(move |s: &mut Shell, _: &mut Tx| s.select(index))
                .build(kit)
                .key(format!("session-{index}")),
        );
    }
    list
}

/// The file tree's slot: the worktree's rows when there is a workspace, and words when there is
/// not, so the region is never blank.
fn files(shell: &Shell, kit: &Kit) -> El {
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
fn workspace(shell: &Shell, kit: &Kit) -> El {
    col()
        .key("workspace")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(kit.c.bg_deep)
        .child(tabs(shell, kit))
        .child(centre(shell, kit))
        .child(rule(kit, "dock-rule").w_full().h(1.0))
        .child(dock(shell, kit))
}

/// The session tabs. Selecting one is the same state swap the rows make.
///
/// The strip grows along its parent's main axis, which is right in a titlebar and wrong in a
/// column, so it gets a row of its own: one tab row's height, the transcript takes the rest.
fn tabs(shell: &Shell, kit: &Kit) -> El {
    let items: Vec<TabItem<'_, Shell>> = shell
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
        .on_select(|shell: &mut Shell, id: &str, _: &mut Tx| {
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
fn centre(shell: &Shell, kit: &Kit) -> El {
    col()
        .key("transcript-column")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(transcript_view(&shell.transcript))
        .child(composer(shell, kit))
}

/// The prompt: one multi-line field, in its own labelled section.
fn composer(shell: &Shell, kit: &Kit) -> El {
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
                .on_edit(|shell: &mut Shell| &mut shell.prompt)
                .on_submit(submit)
                .build(kit),
        )
}

/// Records the question the user wrote as a turn, and empties the box.
///
/// Nothing leaves the window here: the turn is the transcript's, and the agent that owns the turn
/// reads it from there.
fn submit(shell: &mut Shell, _: &mut Tx) {
    let text = shell.prompt.text.trim().to_owned();
    if text.is_empty() {
        return;
    }
    shell.transcript.push_user(text);
    shell.prompt = TextEdit::default();
}

/// The terminal dock, under the transcript and no wider than it.
fn dock(shell: &Shell, kit: &Kit) -> El {
    let tab = shell.active_terminal();
    let (title, state) = match tab {
        Some(tab) => (
            tab.title().to_owned(),
            if tab.frame().is_some() {
                "running"
            } else {
                "waiting"
            },
        ),
        None => (String::new(), "stopped"),
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
    panel.child(dock_body(tab, kit))
}

/// The terminal tabs, in strip order, with the shown one selected.
fn terminal_tabs(shell: &Shell, kit: &Kit) -> El {
    let items: Vec<TabItem<'_, Shell>> = shell
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
        .on_select(|shell: &mut Shell, id: &str, _: &mut Tx| {
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
fn dock_body(tab: Option<&TerminalTab>, kit: &Kit) -> El {
    let Some(frame) = tab.and_then(TerminalTab::frame) else {
        return col()
            .key("terminal-rows")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(note(kit, "no shell running"));
    };
    // Follow the bottom, the way a terminal does: the newest output is the row in view.
    let mut grid = col()
        .key("terminal-rows")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(0.0)
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
