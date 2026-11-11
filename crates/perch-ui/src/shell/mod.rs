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
//!
//! One region per unit: [`layout`] composes the window from [`sidebar`], [`composer`] and
//! [`dock`]; [`state`] holds what every region reads and [`actions`] the presses that change it;
//! [`pump`] folds a frame of the world in; [`keymap`] is the one place a press becomes an
//! [`Intent`] and [`term_keys`] the one place a press becomes bytes for a shell; [`live`] is the
//! seam to the app, and [`paint`] is what a terminal run looks like. [`pane`] is the one value
//! that decides which pane the workspace draws, and [`tabs`] draws the strip that names them all:
//! a session, an opened file, and the settings last, reachable by its tab and by ⌘,.
//!
//! Five presses reach past a region: [`card`] is the key a focused tool card answers, [`paste`]
//! is the clipboard arriving at the app root, [`session`] starts one on the worktree already
//! open, [`folder`] opens a picked folder into a session, and [`editor`] hands the selected file
//! to the app's Zed call.

mod actions;
mod answer;
mod answer_view;
mod card;
mod commands;
mod commands_export;
mod commands_run;
mod commands_session;
mod commands_view;
mod composer;
mod composer_menu;
mod dock;
mod editor;
mod folder;
mod idle;
mod keymap;
mod layout;
mod live;
mod meter;
mod model_picker;
mod model_picker_shell;
mod model_picker_view;
mod paint;
mod palette;
mod palette_take;
mod palette_view;
mod pane;
mod panel_form;
mod panels;
mod paste;
mod pump;
mod session;
mod settings;
mod settings_draft;
mod settings_fields;
mod settings_snapshot;
mod settings_view;
mod sidebar;
mod state;
mod state_build;
mod state_keys;
mod tabs;
mod term_keys;
mod terminal;
mod waiting;

pub use card::window_key;
pub use commands::{COMMANDS, Command};
pub use composer::composer_key;
pub use dock::terminal_key;
pub use idle::Idle;
pub use keymap::{Intent, Typing, intent, typing_intent};
pub use layout::view;
pub use live::{BUDGET, KeyStatus, Live, SHELL_TICK, SettingsSnapshot, TURN_TICK, Waiting};
pub use meter::{context_words, token_words};
pub use palette::{ACTIONS, Action, Row};
pub use pane::Pane;
pub use panels::Panels;
pub use paste::root_paste;
pub use settings::Panel;
pub use settings::settings_key;
pub use state::Shell;
pub use terminal::{EXITED, GRID, TerminalTab};

/// The left column's width, in points.
pub const COLUMN: f32 = 276.0;
/// The terminal dock's height, in points.
const DOCK: f32 = 220.0;
/// The tab strip's row height, in points.
const TABS: f32 = 38.0;
/// The settings tab's id in the strip, which no session may take: the settings are not a session,
/// and the strip draws them after the sessions and the files with the gear's own glyph.
const SETTINGS: &str = "settings";
/// How tall one session row is, in points.
const ROW: f32 = 40.0;
