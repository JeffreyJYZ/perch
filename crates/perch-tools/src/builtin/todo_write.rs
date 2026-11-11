//! `todowrite`: replaces the plan with the whole list the model sends.

use serde_json::{Value, json};

use super::object_schema;
use super::todo::{MAX_TODOS, Status, Todo, replace};
use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec};

pub struct TodoWrite;

impl Tool for TodoWrite {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todowrite".into(),
            description: format!(
                "Keep the plan for a multi-step task. Use it when a task has several steps, and \
                 call it again after each step finishes so the list always says what is done, \
                 what is in progress and what is left. It replaces the whole list: send every \
                 todo each time, in order, and mark at most one `in_progress` — the step being \
                 worked on now. An empty `todos` array clears the list; a list holds at most \
                 {MAX_TODOS}."
            ),
            parameters: object_schema(
                json!({
                    "todos": {
                        "type": "array",
                        "description": "The whole list, in the order it should be read. Sending fewer items removes the rest.",
                        "items": {
                            "type": "object",
                            "description": "One step of the plan.",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "What the step is, in words a person can read."
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "`pending` not started, `in_progress` being worked on (at most one), `completed` done."
                                }
                            },
                            "required": ["content", "status"],
                            "additionalProperties": false
                        }
                    }
                }),
                &["todos"],
                "Replace the todo list.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let todos = read(args)?;
        replace(&ctx.root, todos.clone())?;
        Ok(ToolOutput::text(summary(&todos)))
    }
}

/// The list an argument carries, validated item by item before anything is stored.
fn read(args: &Value) -> Result<Vec<Todo>> {
    let items = args
        .get("todos")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Message("missing array argument `todos`".into()))?;
    if items.len() > MAX_TODOS {
        return Err(Error::Message(format!(
            "at most {MAX_TODOS} todos fit in one list; this one had {}",
            items.len()
        )));
    }
    let mut todos = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let content = item.get("content").and_then(Value::as_str).ok_or_else(|| {
            Error::Message(format!("todo {index} is missing its string `content`"))
        })?;
        let word = item.get("status").and_then(Value::as_str).ok_or_else(|| {
            Error::Message(format!("todo {index} is missing its string `status`"))
        })?;
        todos.push(Todo {
            content: content.to_string(),
            status: Status::parse(word)
                .map_err(|error| Error::Message(format!("todo {index}: {error}")))?,
        });
    }
    Ok(todos)
}

/// The one line the model reads back: `3 todos: 1 done, 1 in progress, 1 pending`.
fn summary(todos: &[Todo]) -> String {
    let count = |status: Status| todos.iter().filter(|todo| todo.status == status).count();
    let noun = if todos.len() == 1 { "todo" } else { "todos" };
    format!(
        "{} {noun}: {} done, {} in progress, {} pending",
        todos.len(),
        count(Status::Completed),
        count(Status::InProgress),
        count(Status::Pending)
    )
}
