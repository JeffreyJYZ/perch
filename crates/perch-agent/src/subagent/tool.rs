//! The tool itself: the delegate, the child's registry, and the parent's cancel.

use std::sync::Arc;

use perch_tools::{Error, Result as ToolResult, Tool, ToolCtx, ToolOutput, ToolRegistry, ToolSpec};
use serde_json::{Value, json};

use super::Delegate;
use super::limits::RESULT_MAX_BYTES;
use super::registry::child_tools;
use super::text::{brief, cut, heading, label};
use crate::cancel::Cancel;

/// The name the model calls this tool by.
pub const TASK: &str = "task";

/// What the model is told: what a child is, what it may not touch, and what it costs.
pub const DESCRIPTION: &str = concat!(
    "Runs one child turn: a fresh agent that starts with only the brief you write and answers its ",
    "final text. Use it for a search or a read that would otherwise fill this conversation, not ",
    "for work that changes files. The child may call only the tools that need no approval — ",
    "`read`, `grep`, `glob`, `list` and the like — so it cannot write, edit, patch or run a shell ",
    "command; do that here instead. It cannot call `task` itself, so a task never has ",
    "grandchildren. The brief must stand alone: the child does not see this conversation, its ",
    "earlier calls or their results. The child has its own round limit; a brief over 64 KiB is ",
    "refused, and an answer over 64 KiB is cut with the cut named."
);

/// The tool: the delegate, the exact tools the child runs with, and the parent's cancel.
pub struct Task {
    delegate: Arc<dyn Delegate>,
    tools: ToolRegistry,
    cancel: Arc<Cancel>,
}

impl Task {
    /// Builds the child's tools first, then the delegate around them.
    ///
    /// The order is the guarantee: the delegate is handed the registry this tool computed, so there
    /// is no moment at which a child could be built around the parent's unfiltered one. A caller
    /// that passes a registry already holding `task` is refused, and the delegate is never built.
    pub fn new<F>(parent: &ToolRegistry, build: F) -> ToolResult<Task>
    where
        F: FnOnce(ToolRegistry) -> Arc<dyn Delegate>,
    {
        let tools = child_tools(parent)?;
        Ok(Task {
            delegate: build(tools.clone()),
            tools,
            cancel: Arc::new(Cancel::default()),
        })
    }

    /// The registry the child runs with, which is what [`Task::new`] handed the delegate.
    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    /// Hands the child the parent turn's cancellation, so one press stops both.
    ///
    /// Without this the tool runs the child under its own counter, which nothing ever bumps: the
    /// shape a test wants, and never the shape an app does.
    #[must_use]
    pub fn cancelled_by(mut self, cancel: Arc<Cancel>) -> Self {
        self.cancel = cancel;
        self
    }
}

impl Tool for Task {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: TASK.to_string(),
            description: DESCRIPTION.to_string(),
            parameters: json!({
                "type": "object",
                "description": "Delegate one self-contained task to a child agent.",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "A label of three to six words, used in the result line."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The whole brief for the child, which starts with no other \
                                        context."
                    }
                },
                "required": ["description", "prompt"],
                "additionalProperties": false,
            }),
        }
    }

    /// The child cannot change anything — its registry holds no tool that would ask a person — so
    /// there is nothing here for a person to approve.
    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> ToolResult<ToolOutput> {
        let description = label(args)?;
        let prompt = brief(args)?;
        // Taken after the arguments are checked, so a refused call never disturbs the counter. A
        // cancel that landed before this call began belongs to a turn the loop has already
        // stopped; one that lands after it now ends the child.
        let generation = self.cancel.begin();
        let answer = self.delegate.run(&prompt, ctx, &self.cancel);
        if self.cancel.stopped(generation) {
            return Err(Error::Message(format!(
                "task \"{description}\" was cancelled, so the child produced no answer"
            )));
        }
        let answer = answer
            .map_err(|why| Error::Message(format!("task \"{description}\" failed: {why}")))?;
        if answer.trim().is_empty() {
            return Err(Error::Message(format!(
                "task \"{description}\" finished without an answer, so there is no result to read"
            )));
        }
        let heading = heading(&description, self.delegate.rounds());
        Ok(ToolOutput::text(format!(
            "{heading}\n\n{}",
            cut(answer, RESULT_MAX_BYTES)
        )))
    }
}
