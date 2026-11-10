//! What a press on a panel control does.
//!
//! A panel is data, so a control the host draws is only a name until its plugin answers for it.
//! This is the one path a press takes: the host proves the control is one the plugin declares
//! right now, hands the guest the id and the value, and treats a trap exactly as a trapping tool
//! is treated — the plugin is disabled here and the press comes back as an error, never as an
//! unwind through the window.
//!
//! Reading the panel to prove the control is declared is a second guest call per press, and it is
//! the only way the host can say "it never declared that" without keeping a copy that could be
//! one panel behind.

use perch_tools::{Error, Result};
use wasmtime::Result as WasmResult;

use crate::bounds::{arm, lock};
use crate::host::PluginHost;
use crate::panel::PanelModel;
use crate::plugin::PluginInner;

impl PluginHost {
    /// Reports a press on `control` to `plugin`, with the text an input was submitted with.
    ///
    /// The panel is read first, so a press for a control the plugin never declared is answered
    /// with words instead of being forwarded to a guest that would ignore it. A plugin that is
    /// gone — unloaded, or disabled by an earlier trap — answers the same way. A guest that traps
    /// while being asked is disabled here, and every later call refuses.
    pub fn panel_action(&self, plugin: &str, control: &str, value: Option<&str>) -> Result<()> {
        let Some(loaded) = self
            .plugins
            .iter()
            .find(|candidate| candidate.name == plugin)
        else {
            return Err(Error::Message(format!(
                "plugin `{plugin}` is not loaded, so `{control}` reached no plugin"
            )));
        };
        if loaded.is_disabled() {
            return Err(Error::Message(format!(
                "plugin `{plugin}` is disabled, so `{control}` reached no plugin"
            )));
        }

        let mut inner = lock(&loaded.inner);
        // A trap may be noticed here, in the call that proves the control exists, so the
        // disabling has to happen on both failures rather than only on the action's own.
        let model = match panel(&mut inner) {
            Ok(model) => model,
            Err(err) => {
                inner.disabled = true;
                return Err(trapped(plugin, control, err));
            }
        };
        if !model.has_control(control) {
            return Err(Error::Message(format!(
                "plugin `{plugin}` declares no control `{control}`, so nothing was reported"
            )));
        }

        let outcome = {
            let PluginInner {
                store, bindings, ..
            } = &mut *inner;
            match arm(store) {
                Ok(()) => bindings
                    .perch_plugin_actions()
                    .call_action(store, control, value),
                Err(err) => Err(err),
            }
        };
        match outcome {
            Ok(()) => Ok(()),
            Err(err) => {
                inner.disabled = true;
                Err(trapped(plugin, control, err))
            }
        }
    }
}

/// What the guest says its panel is now, from an already locked store.
fn panel(inner: &mut PluginInner) -> WasmResult<PanelModel> {
    let PluginInner {
        store, bindings, ..
    } = inner;
    arm(store)?;
    bindings
        .perch_plugin_panels()
        .call_panel(store)
        .map(PanelModel::from_wit)
}

/// The one error a trap produces, naming the plugin that went and the control it was asked about.
fn trapped(plugin: &str, control: &str, err: wasmtime::Error) -> Error {
    Error::Message(format!(
        "plugin `{plugin}` was disabled while answering `{control}`: {err}"
    ))
}
