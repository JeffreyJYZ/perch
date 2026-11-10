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
//!
//! The update offer is drawn by the same column and pressed the same way, but it belongs to the
//! app: its press is answered here, by asking the update thread to stage an archive.

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
        // The update offer is drawn by the same column and pressed the same way, but it is not a
        // plugin: the press stages a download instead of reaching a guest.
        if plugin == crate::update::OWNER {
            return self.update_action(control);
        }
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

    /// A press on the update panel: only a standing offer can stage anything.
    ///
    /// A press while the check is still out is refused in words rather than queued for a release
    /// nothing named yet, and a press with nothing on offer is refused the same way. Staging is
    /// all it does: the archive lands on disk and the person installs it after perch quits.
    fn update_action(&self, control: &str) -> std::result::Result<(), String> {
        use crate::update::{STAGE, Status};

        let Some(worker) = self.update.as_ref() else {
            return Err(
                "no update check is configured, so there is nothing to download".to_string(),
            );
        };
        match self.update_status.as_deref() {
            Some(Status::Offer { .. }) if control == STAGE => {
                worker.stage();
                Ok(())
            }
            Some(Status::Offer { .. }) => Err(format!(
                "the update panel declares no control `{control}`, so nothing was reported"
            )),
            Some(Status::Checking) => Err(
                "the update check has not finished, so there is nothing to download yet"
                    .to_string(),
            ),
            _ => Err("there is no update on offer, so nothing was downloaded".to_string()),
        }
    }
}
