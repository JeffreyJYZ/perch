//! One registered tool, and what a guest that traps costs it.

use std::sync::{Arc, Mutex};

use perch_tools::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::Value;

use crate::bounds::{arm, lock};
use crate::plugin::PluginInner;

/// One tool a plugin declared, registered through the same path a built-in takes.
pub struct PluginTool {
    pub(crate) plugin: String,
    pub(crate) name: String,
    pub(crate) spec: ToolSpec,
    pub(crate) inner: Arc<Mutex<PluginInner>>,
}

impl Tool for PluginTool {
    fn spec(&self) -> ToolSpec {
        // The real spec, disabled or not. A tool that cannot run is removed from the registry by
        // `PluginHost::release`, never hidden behind a spec with no name.
        self.spec.clone()
    }

    /// A plugin tool never needs approval: the ABI gives it no way to touch anything.
    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let args_json = serde_json::to_string(args)
            .map_err(|err| Error::Message(format!("arguments are not JSON: {err}")))?;

        let mut inner = lock(&self.inner);
        if inner.disabled {
            return Err(disabled_error(&self.plugin, &self.name));
        }

        let outcome = {
            let PluginInner {
                store, bindings, ..
            } = &mut *inner;
            match arm(store) {
                Ok(()) => bindings
                    .perch_plugin_tools()
                    .call_call(store, &self.name, &args_json),
                Err(err) => Err(err),
            }
        };

        match outcome {
            Ok(Ok(text)) => Ok(ToolOutput::text(text)),
            Ok(Err(message)) => Err(Error::Message(message)),
            Err(err) => {
                // Fuel exhaustion, the epoch deadline and a wasm trap all land here. Disabling is
                // the whole point: a guest that faults must not be called again, and the error is
                // returned as text rather than unwinding through the caller.
                inner.disabled = true;
                Err(Error::Message(format!(
                    "plugin `{}` was disabled after `{}` trapped: {err}",
                    self.plugin, self.name
                )))
            }
        }
    }
}

fn disabled_error(plugin: &str, tool: &str) -> Error {
    Error::Message(format!(
        "plugin `{plugin}` is disabled, so `{tool}` is gone"
    ))
}
