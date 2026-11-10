//! The WIT world every plugin is compiled against, and the wasmtime bindings generated from it.
//!
//! The host side of that world is deliberately tiny: a plugin may call `log` and nothing else.
//! There is no read, no exec and no network import, so a plugin that wants the filesystem has no
//! function to ask through — the refusal lives in the ABI, not in a check the host could forget.

use wasmtime::StoreLimits;
use wasmtime::component::ResourceTable;
use wasmtime::component::bindgen;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

bindgen!({
    path: "wit",
    world: "perch-plugin",
});

/// Per-store host state: WASI with no preopened directory, and the store's resource caps.
pub struct Data {
    ctx: WasiCtx,
    table: ResourceTable,
    /// Read by the store's resource limiter; `StoreLimits` owns the memory cap.
    pub limits: StoreLimits,
    /// `log` lines from the guest. Nothing surfaces them yet.
    #[allow(dead_code)]
    logs: Vec<String>,
}

impl Data {
    pub fn new(limits: StoreLimits) -> Self {
        Self {
            ctx: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            limits,
            logs: Vec::new(),
        }
    }
}

impl WasiView for Data {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl perch::plugin::log::Host for Data {
    fn log(&mut self, message: String) {
        self.logs.push(message);
    }
}
