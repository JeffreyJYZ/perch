//! The state a panel's controls need: the text each input holds, and the last refusal.
//!
//! A plugin owns what a press means; the shell owns what a person typed into a field the plugin
//! only named, and the words for a press that reached no plugin at all. Both live here rather
//! than on [`Shell`], so the shell itself carries one field for the whole column.
//!
//! The key is the plugin and the control together: two plugins may both declare `note`, and they
//! must never share one field.

use std::collections::HashMap;

use rune::TextEdit;

use super::live::Live;
use super::panels::Panels;
use super::state::Shell;

/// What every panel input holds, and what the last press that changed nothing said.
#[derive(Default)]
pub(super) struct PanelForm {
    drafts: HashMap<(String, String), TextEdit>,
    /// The words of the last press that reached no plugin, shown in the column until the next
    /// press. A refused press must not look like one that worked, which is what silence means.
    refusal: Option<String>,
}

impl PanelForm {
    /// The text `plugin`'s field for `control` holds now. A field never typed into is empty.
    pub(super) fn draft(&self, plugin: &str, control: &str) -> TextEdit {
        self.drafts
            .get(&(plugin.to_string(), control.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// The field itself, for the editor to own while a person types in it.
    pub(super) fn draft_mut(&mut self, plugin: &str, control: &str) -> &mut TextEdit {
        self.drafts
            .entry((plugin.to_string(), control.to_string()))
            .or_default()
    }

    /// The words of the last refused press, when there is one.
    pub(crate) fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }
}

impl<L: Live + Panels> Shell<L> {
    /// The text `plugin`'s field for `control` holds now, as the field draws it.
    pub(crate) fn panel_draft(&self, plugin: &str, control: &str) -> TextEdit {
        self.panel.draft(plugin, control)
    }

    /// The field itself, for the editor to own while a person types in it.
    pub(crate) fn panel_draft_mut(&mut self, plugin: &str, control: &str) -> &mut TextEdit {
        self.panel.draft_mut(plugin, control)
    }

    /// Reports a button press: a button has no value to report, so the press is the whole event.
    pub(crate) fn panel_press(&mut self, plugin: &str, control: &str) {
        self.panel_report(plugin, control, None);
    }

    /// Reports an input submit, with the text the field holds right now.
    pub(crate) fn panel_submit(&mut self, plugin: &str, control: &str) {
        let value = self.panel.draft(plugin, control).text.clone();
        self.panel_report(plugin, control, Some(value));
    }

    /// The one path a press takes: through the app, which owns the plugin host. The app answers
    /// nothing on a queue, so the column is where a plugin's own refusal is said; what lands here
    /// is a press this window could not even hand over.
    fn panel_report(&mut self, plugin: &str, control: &str, value: Option<String>) {
        let outcome = match self.live.as_mut() {
            Some(live) => live.panel_action(plugin, control, value),
            None => Err("no app is running, so the press reached no plugin".to_string()),
        };
        self.panel.refusal = outcome.err();
    }
}
