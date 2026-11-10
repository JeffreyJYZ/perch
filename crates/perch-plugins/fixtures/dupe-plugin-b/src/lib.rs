//! Second of two fixture plugins that declare the same tool name.
//!
//! `second_only` comes first in the spec list on purpose: it does not collide, so a host that
//! validated and registered one name at a time would leave it behind when `duplicated` fails.
//! Registering nothing is the whole plugin's answer. Its panel is empty: this fixture exists for
//! the collision, not for a panel.

wit_bindgen::generate!({
    path: "../../wit",
    world: "perch-plugin",
});

use exports::perch::plugin::actions::Guest as ActionsGuest;
use exports::perch::plugin::panels::{Guest as PanelsGuest, PanelModel};
use exports::perch::plugin::tools::{Guest as ToolsGuest, ToolSpec};

struct DupePluginB;

impl ToolsGuest for DupePluginB {
    fn tool_specs() -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "second_only".to_string(),
                description: "A name nothing else takes.".to_string(),
                parameters_json: r#"{"type":"object","properties":{}}"#.to_string(),
            },
            ToolSpec {
                name: "duplicated".to_string(),
                description: "The second plugin to declare `duplicated`.".to_string(),
                parameters_json: r#"{"type":"object","properties":{}}"#.to_string(),
            },
        ]
    }

    fn call(name: String, _args: String) -> Result<String, String> {
        match name.as_str() {
            "second_only" | "duplicated" => Ok("second".to_string()),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

impl PanelsGuest for DupePluginB {
    fn panel() -> PanelModel {
        PanelModel {
            title: String::new(),
            sections: Vec::new(),
        }
    }

    fn on_event(_event_json: String) {}
}

/// No controls are declared, so there is nothing to answer.
impl ActionsGuest for DupePluginB {
    fn action(_id: String, _value: Option<String>) {}
}

export!(DupePluginB);
