//! A press on a control a plugin's panel declared.
//!
//! The plugin host lives on the worker's thread, so a press cannot call it and return an answer
//! in the same breath. What can be answered here is the question the press is really asking: does
//! the panel the window is drawing declare this control, and is its plugin still there? That is
//! checked against the snapshot the column was laid out from, so a press on a control that has
//! gone is words in the column rather than work queued for a guest that would ignore it.
//!
//! A press that passes goes to the worker. The panel it changes is re-read into the snapshot that
//! follows, which is what makes the change visible without the window ever waiting on a guest —
//! and a guest that traps while answering is disabled by the host, exactly as a trapping tool is.

use crate::AppState;

impl AppState {
    /// Reports a press on `control`, which `plugin`'s panel declared, with the text an input was
    /// submitted with. A button press has no value.
    ///
    /// The error is why the press reached no plugin, in words the column can show. It is refused
    /// here only when the drawn panel cannot address it; the host refuses the rest on its own
    /// thread, and says so in the column's words.
    pub fn panel_action(
        &self,
        plugin: &str,
        control: &str,
        value: Option<String>,
    ) -> std::result::Result<(), String> {
        let Some(entry) = self
            .snapshot
            .panels
            .iter()
            .find(|entry| entry.plugin == plugin)
        else {
            return Err(format!(
                "plugin `{plugin}` is not drawing a panel, so `{control}` reached no plugin"
            ));
        };
        if !entry.model.has_control(control) {
            return Err(format!(
                "plugin `{plugin}` declares no control `{control}`, so nothing was reported"
            ));
        }
        self.worker.panel_action(plugin, control, value);
        Ok(())
    }
}
