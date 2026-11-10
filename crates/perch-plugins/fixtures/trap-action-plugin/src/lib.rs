//! Fixture plugin whose `action` traps.
//!
//! A press is answered on the UI's own thread, so a guest that faults while answering one must be
//! disabled exactly as a trapping tool or a trapping panel disables it, and the press must come
//! back as an error rather than unwinding through the window. Its panel is valid and its `fine`
//! tool never faults, which proves only this plugin was taken.

wit_bindgen::generate!({
    path: "../../wit",
    world: "perch-plugin",
});

use exports::perch::plugin::actions::Guest as ActionsGuest;
use exports::perch::plugin::panels::{
    Guest as PanelsGuest, PanelButton, PanelModel, PanelSection,
};
use exports::perch::plugin::tools::{Guest as ToolsGuest, ToolSpec};

struct TrapActionPlugin;

impl ToolsGuest for TrapActionPlugin {
    fn tool_specs() -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "fine".to_string(),
            description: "A tool that never faults, so the host can be seen still working."
                .to_string(),
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

impl PanelsGuest for TrapActionPlugin {
    /// A real panel with a real control, so the host draws a button and a press reaches `action`.
    fn panel() -> PanelModel {
        PanelModel {
            title: "Trap action".to_string(),
            sections: vec![
                PanelSection::Text("press the button".to_string()),
                PanelSection::Button(PanelButton {
                    id: "trap".to_string(),
                    label: "Trap".to_string(),
                }),
            ],
        }
    }

    fn on_event(_event_json: String) {}
}

impl ActionsGuest for TrapActionPlugin {
    /// A real trap, not a returned error: the host must see a wasm trap here.
    fn action(_id: String, _value: Option<String>) {
        core::arch::wasm32::unreachable()
    }
}

export!(TrapActionPlugin);
