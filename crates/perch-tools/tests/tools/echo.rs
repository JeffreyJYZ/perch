//! A tiny tool defined only here, proving registration is dynamic rather than a fixed list.

use perch_tools::{Result, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

pub struct Echo;

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Echo the given text back.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let text = args["text"].as_str().unwrap_or_default().to_string();
        Ok(ToolOutput {
            text,
            preview: None,
        })
    }
}
