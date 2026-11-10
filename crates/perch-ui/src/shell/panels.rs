//! The plugin column: one panel per loaded plugin, and the words for the ones that are not there.
//!
//! A plugin's panel is data the app decoded ([`PanelEntry`]); [`panel_view`] is the only thing that
//! turns it into elements, so this module decides only where the column goes and whether it exists.
//! It does not exist when the app has no plugin and nothing to say about one — the window a person
//! sees without plugins is the window they had before there were any.
//!
//! [`Panels`] is what the window asks the app for. It is a second trait beside
//! [`Live`](super::live::Live) because a panel is not a transcript or a prompt: an app with no
//! plugin host at all answers neither method and draws no column, which is the truth. A press on a
//! control in a panel is the exception that goes the other way: it is a [`Panels`] method because
//! the plugin host is what it must reach.

use perch_plugins::PanelEntry;
use rune::{El, col};
use rune_kit::Kit;

use super::live::{Idle, Live};
use super::state::Shell;
use crate::files::{note, section_label};
use crate::plugin_panel::panel_view;

/// The column's width, in points.
pub(super) const PANEL: f32 = 260.0;

/// What the window asks of the app for the plugin column.
pub trait Panels {
    /// The panels to draw, in load order. Empty when no plugin is loaded.
    fn panels(&self) -> Vec<PanelEntry> {
        Vec::new()
    }

    /// What went wrong with a plugin, in words: a file that did not load, a plugin that trapped.
    ///
    /// The column is the one region a person can look at without a session open, so a load failure
    /// said here is said where it is still readable an hour later.
    fn plugin_words(&self) -> Vec<String> {
        Vec::new()
    }

    /// Reports a press on `control`, which `plugin`'s panel declared, with the text an input was
    /// submitted with. A button press has no value.
    ///
    /// The error is why the press reached no plugin — a plugin that is gone, or an id it does not
    /// declare — and it is in words because the column says it.
    fn panel_action(
        &mut self,
        _plugin: &str,
        _control: &str,
        _value: Option<String>,
    ) -> std::result::Result<(), String> {
        Err("no app is running, so the press reached no plugin".to_string())
    }
}

/// A window with no app behind it has no plugin host, so it has no column and no press to report.
impl Panels for Idle {}

/// The column, or `None` when there is nothing to draw and nothing to say.
///
/// Every control a panel declares is attached here: the panel view draws the button or the field
/// and this is where its press reaches the app, so a plugin's answer is one pump away rather than
/// a handler of the renderer's.
pub(super) fn column<L: Live + Panels>(shell: &Shell<L>, kit: &Kit) -> Option<El> {
    let live = shell.live()?;
    let panels = live.panels();
    let words = live.plugin_words();
    let refused = shell.panel.refusal();
    if panels.is_empty() && words.is_empty() && refused.is_none() {
        return None;
    }
    let mut body = col()
        .key("plugins")
        .role("region")
        .label("Plugins")
        .w(PANEL)
        .h_full()
        .min_h(0.0)
        .fill(kit.c.bg_base)
        .px(10.0)
        .pt(12.0)
        .pb(10.0)
        .gap(8.0)
        .child(section_label(kit, "Plugins", "plugins-label"));
    for word in &words {
        body = body.child(note(kit, word));
    }
    for entry in &panels {
        body = body.child(
            panel_view(shell, &entry.plugin, kit, &entry.model)
                .key(format!("plugin-{}", entry.plugin)),
        );
    }
    if let Some(refusal) = refused {
        body = body.child(note(kit, refusal));
    }
    Some(body)
}
