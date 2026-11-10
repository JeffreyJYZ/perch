//! Fixture plugin whose `panel` traps.
//!
//! A panel is drawn on the UI's own thread, so a guest that faults while producing one would
//! take the window down if the host simply unwrapped it. The host must treat it exactly like a
//! trapping tool: disable this plugin and answer `None`. Its `fine` tool proves the host is
//! still alive and that only this plugin was affected.

wit_bindgen::generate!({
    path: "../../wit",
    world: "perch-plugin",
});

use exports::perch::plugin::panels::{Guest as PanelsGuest, PanelModel};
use exports::perch::plugin::tools::{Guest as ToolsGuest, ToolSpec};

struct TrapPanelPlugin;

impl ToolsGuest for TrapPanelPlugin {
    fn tool_specs() -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "fine".to_string(),
            description: "A tool that never faults, so the host can be seen still working.".to_string(),
            parameters_json: r#"{"type":"object","properties":{}}"#.to_string(),
        }]
    }

    fn call(name: String, _args: String) -> Result<String, String> {
        match name.as_str() {
            "fine" => Ok("fine".to_string()),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

impl PanelsGuest for TrapPanelPlugin {
    fn panel() -> PanelModel {
        // A real trap, not a returned error: the host must see a wasm trap here.
        core::arch::wasm32::unreachable()
    }

    fn on_event(_event_json: String) {}
}

export!(TrapPanelPlugin);
