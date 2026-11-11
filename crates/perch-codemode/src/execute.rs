//! The `execute` tool: one program in, one JSON result out, and every budget checked here.

use std::sync::Arc;

use perch_tools::{Result, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::budget::{Budget, Cancel, Limits};
use crate::host::ToolHost;
use crate::js::{self, Outcome};
use crate::output::Out;

/// The name the model calls this tool by.
pub const EXECUTE: &str = "execute";

/// What the model is told: when Code Mode pays, what it may use, and what it costs.
pub const DESCRIPTION: &str = concat!(
    "Runs a JavaScript program that calls the other tools from inside it, so several calls and ",
    "the work between them cost one tool call in the conversation. Reach for it when a task ",
    "needs more than one tool call in a row, when results need looping over or filtering before ",
    "you read them, or when a tool's output is heavy: reduce it inside the program and `print` ",
    "only what you keep, instead of dumping the whole thing into the conversation.\n\n",
    "Inside the program: `tools.call(name, args)` runs a tool and answers `{ text }` (plus ",
    "`preview` when the tool has one); `args` is an object or a JSON string. A failed or unknown ",
    "tool throws an `Error` you can `try`/`catch`. `tools.list()` answers `[{ name, description }]`. ",
    "`print(...values)` adds a line to the result, and the program's last expression is reported ",
    "when it is not `undefined`. It is ordinary JavaScript: `const`, arrow functions, ",
    "`map`/`filter`, `try`/`catch`, template literals.\n\n",
    "A tool that asks a person before it runs cannot be called here: call that tool directly so ",
    "the change can be approved. Each execution gets 30 s of wall clock, 32 tool calls and ",
    "64 KiB of output, and is stopped with a plain error naming whichever runs out."
);

/// The tool itself: it holds the host, and spends it under a budget per call.
pub struct Execute {
    host: Arc<dyn ToolHost>,
    limits: Limits,
    cancel: Option<Arc<dyn Cancel>>,
}

impl Execute {
    pub fn new(host: Arc<dyn ToolHost>) -> Self {
        Self {
            host,
            limits: Limits::default(),
            cancel: None,
        }
    }

    /// The numbers one execution runs under; a test shrinks them.
    pub fn accepting(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// The turn's cancellation, so stopping the turn stops the program, not just the wait.
    pub fn cancelled_by(mut self, cancel: Arc<dyn Cancel>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    fn run(&self, program: &str, ctx: &ToolCtx) -> String {
        let budget = Budget::start(self.limits, self.cancel.clone());
        let out = Out::new(self.limits.output);
        let finished = js::run(
            program,
            self.host.clone(),
            ctx.clone(),
            budget,
            out,
        );
        match finished.outcome {
            Outcome::Ok => success(finished.out.text()),
            Outcome::Stopped(stop) => failure(&stop.words(), finished.out.text()),
            Outcome::Threw { message, stack } => failure(&message, &with_output(&finished.out, &stack)),
            Outcome::Unparsed {
                message,
                diagnostic,
            } => failure(&message, &diagnostic),
        }
    }
}

impl Tool for Execute {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: EXECUTE.to_string(),
            description: DESCRIPTION.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "program": {
                        "type": "string",
                        "description": "The JavaScript program to run."
                    }
                },
                "required": ["program"]
            }),
        }
    }

    /// A program can only reach tools that need no approval — the interpreter and the host both
    /// refuse the rest — so there is nothing here for a person to approve.
    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let Some(program) = args.get("program").and_then(Value::as_str) else {
            return Ok(ToolOutput::text(failure(
                "the execute tool needs a `program` string",
                "",
            )));
        };
        Ok(ToolOutput::text(self.run(program, ctx)))
    }
}

/// The result a program that finished inside every budget answers with.
fn success(output: &str) -> String {
    json!({ "ok": true, "output": output }).to_string()
}

/// The result a stopped execution answers with: what was wrong, and the output so far.
fn failure(error: &str, diagnostic: &str) -> String {
    json!({ "ok": false, "error": error, "diagnostic": diagnostic }).to_string()
}

/// What a program printed before it threw, then the throw itself.
fn with_output(out: &Out, detail: &str) -> String {
    if out.is_empty() {
        detail.to_string()
    } else {
        format!("{}\n{detail}", out.text())
    }
}
