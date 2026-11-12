//! What both of `execute`'s test targets need: a context, one run, and a field of the answer.

use perch_codemode::Execute;
use perch_tools::{Tool, ToolCtx};
use serde_json::{Value, json};

pub fn ctx() -> ToolCtx {
    ToolCtx {
        root: std::env::temp_dir(),
    }
}

/// One program, run once, as the JSON the model would read.
pub fn result(tool: &Execute, program: &str) -> Value {
    let output = tool
        .call(&json!({ "program": program }), &ctx())
        .expect("the tool answers even when the program is wrong");
    serde_json::from_str(&output.text).expect("the tool answers with JSON")
}

pub fn field(answer: &Value, key: &str) -> String {
    answer[key].as_str().unwrap_or_default().to_string()
}
