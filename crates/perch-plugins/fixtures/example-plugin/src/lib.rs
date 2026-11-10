//! Fixture plugin for the perch plugin host tests.
//!
//! Three tools, each pinning one rule: `echo` proves registration and a round trip, `explode`
//! traps so the host must disable the plugin, and `read_outside` genuinely tries to read a file
//! the host never granted, so the refusal is proved by the ABI rather than by the guest being
//! polite.

wit_bindgen::generate!({
    path: "../../wit",
    world: "perch-plugin",
});

use exports::perch::plugin::tools::{Guest, ToolSpec};

struct ExamplePlugin;

fn spec(name: &str, description: &str, parameters: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters_json: parameters.to_string(),
    }
}

impl Guest for ExamplePlugin {
    fn tool_specs() -> Vec<ToolSpec> {
        vec![
            spec(
                "echo",
                "Echo the given text back.",
                r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
            ),
            spec(
                "explode",
                "Trap, so the host has to disable this plugin.",
                r#"{"type":"object","properties":{}}"#,
            ),
            spec(
                "read_outside",
                "Try to read a file the host never granted.",
                r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#,
            ),
        ]
    }

    fn call(name: String, args: String) -> Result<String, String> {
        let args: serde_json::Value =
            serde_json::from_str(&args).map_err(|err| format!("arguments are not JSON: {err}"))?;

        match name.as_str() {
            "echo" => Ok(args
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()),
            // A real trap, not a returned error: the host must see a wasm trap here.
            "explode" => core::arch::wasm32::unreachable(),
            "read_outside" => {
                let path = args
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("/etc/hosts");
                // Succeeds only if the host ever hands this plugin a filesystem it can see.
                std::fs::read_to_string(path)
                    .map_err(|err| format!("the plugin host grants no file access: {err}"))
            }
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

export!(ExamplePlugin);
