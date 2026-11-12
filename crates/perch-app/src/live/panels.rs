//! The plugin column as [`Panels`] sees it: the panels and words off the worker's last snapshot,
//! plus the update offer, so a frame reads them instead of asking a plugin while it draws.
//!
//! Split from `live.rs` because it is a different trait on the same app and because the window can
//! read this column with no session open — the one region where an update is exactly the news.

use perch_plugins::PanelEntry;
use perch_ui::shell::Panels;

use crate::AppState;

impl Panels for AppState {
    /// The plugin panels, then the update offer: the column is the one region a person can read
    /// without a session open, and an update is exactly that kind of news.
    fn panels(&self) -> Vec<PanelEntry> {
        let mut panels = self.snapshot.panels.clone();
        if let Some(status) = self.update_status.as_ref() {
            panels.push(status.panel());
        }
        panels
    }

    fn plugin_words(&self) -> Vec<String> {
        self.snapshot.words.clone()
    }

    /// A press on a panel control: refused here when the drawn panel cannot address it, and
    /// otherwise queued for the worker that owns the plugin host. The panel it changes is re-read
    /// into the next snapshot, which is what draws the change.
    fn panel_action(
        &mut self,
        plugin: &str,
        control: &str,
        value: Option<String>,
    ) -> std::result::Result<(), String> {
        AppState::panel_action(self, plugin, control, value)
    }
}
