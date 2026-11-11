//! The window's plugin column: one panel per live plugin, then the words for what is not there.
//!
//! Kept apart from the [`Live`](perch_ui::shell::Live) half so each file describes one seam.

use perch_plugins::PanelEntry;
use perch_ui::shell::Panels;

use crate::window::Window;

impl Panels for Window {
    fn panels(&self) -> Vec<PanelEntry> {
        self.app().map(Panels::panels).unwrap_or_default()
    }

    fn plugin_words(&self) -> Vec<String> {
        self.app().map(Panels::plugin_words).unwrap_or_default()
    }

    fn panel_action(
        &mut self,
        plugin: &str,
        control: &str,
        value: Option<String>,
    ) -> std::result::Result<(), String> {
        match self {
            Window::Open(app) => Panels::panel_action(app.as_mut(), plugin, control, value),
            Window::Offline(why) => Err(format!(
                "no app is running, so `{control}` reached no plugin: {why}"
            )),
        }
    }
}
