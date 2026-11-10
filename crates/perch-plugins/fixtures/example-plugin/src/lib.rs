//! Fixture plugin for the perch plugin host tests.
//!
//! Three tools pin the tool rules: `echo` proves registration and a round trip, `explode` traps
//! so the host must disable the plugin, and `read_outside` genuinely tries to read a file the
//! host never granted, so the refusal is proved by the ABI rather than by the guest being
//! polite.
//!
//! Its panel is a counter: `panel` reports how many turns this store has seen end and
//! `on-event` counts them, so a test proves an event reached the guest by watching the panel
//! change. A fresh store starts at zero, so every test sees the same first panel.

wit_bindgen::generate!({
    path: "../../wit",
    world: "perch-plugin",
});

use std::sync::atomic::{AtomicU32, Ordering};

use exports::perch::plugin::panels::{
    Guest as PanelsGuest, PanelButton, PanelInput, PanelModel, PanelSection,
};
use exports::perch::plugin::tools::{Guest as ToolsGuest, ToolSpec};

struct CounterPlugin;

/// Turns this store has seen end. Per store, so one load never sees another's count.
static TURNS: AtomicU32 = AtomicU32::new(0);

fn spec(name: &str, description: &str, parameters: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters_json: parameters.to_string(),
    }
}

impl ToolsGuest for CounterPlugin {
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

impl PanelsGuest for CounterPlugin {
    fn panel() -> PanelModel {
        PanelModel {
            title: "Counter".to_string(),
            // Text only, so the flattened lines are exactly the count; the button and the
            // input are host-drawn controls and are never part of the text lines.
            sections: vec![
                PanelSection::Text(format!("count: {}", TURNS.load(Ordering::Relaxed))),
                PanelSection::Button(PanelButton {
                    id: "reset".to_string(),
                    label: "Reset".to_string(),
                }),
                PanelSection::Input(PanelInput {
                    id: "note".to_string(),
                    label: "Note".to_string(),
                }),
            ],
        }
    }

    fn on_event(event_json: String) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&event_json) else {
            return;
        };
        if event.get("kind").and_then(serde_json::Value::as_str) == Some("turn_end") {
            TURNS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

export!(CounterPlugin);
