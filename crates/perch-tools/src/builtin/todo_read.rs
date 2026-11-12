//! `todoread`: the plan as JSON, or the words that there is none.

use serde_json::{Value, json};

use super::object_schema;
use super::todo::todos;
use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec};

pub struct TodoRead;

impl Tool for TodoRead {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todoread".into(),
            description: "The todo list this session wrote with `todowrite`, as the same JSON \
                          shape, or the words that there are none. Read it when the plan is not \
                          in view — after a compaction, or before deciding the next step of a \
                          multi-step task."
                .into(),
            parameters: object_schema(json!({}), &[], "Read the todo list."),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, _args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let todos = todos(&ctx.root);
        if todos.is_empty() {
            return Ok(ToolOutput::text("no todos"));
        }
        let text = serde_json::to_string(&todos)
            .map_err(|error| Error::Message(format!("the todo list did not encode: {error}")))?;
        Ok(ToolOutput::text(text))
    }
}
