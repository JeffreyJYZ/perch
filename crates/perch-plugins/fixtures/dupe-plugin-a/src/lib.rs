//! First of two fixture plugins that declare the same tool name.
//!
//! Loaded first, so `duplicated` belongs to this plugin; the host must keep it when the second
//! plugin's declaration collides. `call` answers `first`, which is how a test tells the two apart.

wit_bindgen::generate!({
    path: "../../wit",
    world: "perch-plugin",
});

use exports::perch::plugin::tools::{Guest, ToolSpec};

struct DupePluginA;

impl Guest for DupePluginA {
    fn tool_specs() -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "duplicated".to_string(),
            description: "The first plugin to declare `duplicated`.".to_string(),
            parameters_json: r#"{"type":"object","properties":{}}"#.to_string(),
        }]
    }

    fn call(name: String, _args: String) -> Result<String, String> {
        match name.as_str() {
            "duplicated" => Ok("first".to_string()),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

export!(DupePluginA);
