//! The fixtures: a delegate that only remembers, a parent registry, and the arguments shape.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use perch_agent::cancel::Cancel;
use perch_agent::subagent::Delegate;
use perch_agent::{Dispatcher, Task};
use perch_tools::{Tool, ToolCtx, ToolOutput, ToolRegistry, ToolSpec};
use serde_json::{Value, json};

/// A delegate that answers what it was told, records what its child was handed, and counts runs.
///
/// The count is the point: "the delegate was never called" is a number here rather than an absence
/// somebody has to trust.
pub struct Memory {
    answer: std::result::Result<String, String>,
    rounds: Option<usize>,
    calls: AtomicUsize,
    /// Whether the child cancels its parent while it runs, which is how a press mid-run is modelled.
    cancels: bool,
    /// The tool names the child was given, recorded when the delegate was built.
    child: Mutex<Vec<String>>,
}

impl Memory {
    /// A child that answers `text`.
    pub fn ok(text: &str) -> Self {
        Self {
            answer: Ok(text.to_string()),
            rounds: None,
            calls: AtomicUsize::new(0),
            cancels: false,
            child: Mutex::new(Vec::new()),
        }
    }

    /// A child that fails for `why`.
    pub fn err(why: &str) -> Self {
        Self {
            answer: Err(why.to_string()),
            ..Self::ok("")
        }
    }

    /// A child that takes `rounds` to answer.
    #[must_use]
    pub fn in_rounds(mut self, rounds: usize) -> Self {
        self.rounds = Some(rounds);
        self
    }

    /// A child that presses cancel on the turn that started it, then answers anyway.
    #[must_use]
    pub fn cancelling(mut self) -> Self {
        self.cancels = true;
        self
    }

    /// How many times the child was run.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// The tool names the child was handed, in the order it received them.
    pub fn child(&self) -> Vec<String> {
        self.child.lock().unwrap().clone()
    }

    fn remember(&self, tools: &ToolRegistry) {
        let mut child = self.child.lock().unwrap();
        *child = tools.specs().iter().map(|spec| spec.name.clone()).collect();
    }
}

impl Delegate for Memory {
    fn run(
        &self,
        _task: &str,
        _ctx: &ToolCtx,
        cancel: &Cancel,
    ) -> std::result::Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.cancels {
            cancel.cancel();
        }
        self.answer.clone()
    }

    fn rounds(&self) -> Option<usize> {
        self.rounds
    }
}

/// The tool, plus the handle its delegate records through.
pub fn tool(registry: &ToolRegistry, memory: Memory) -> (Task, Arc<Memory>) {
    let memory = Arc::new(memory);
    let delegate = Arc::clone(&memory);
    let task = Task::new(registry, move |tools| {
        delegate.remember(&tools);
        delegate
    })
    .expect("the built-in registry holds no `task`");
    (task, memory)
}

/// The registry a session really builds: the built-ins plus `ask_user`.
pub fn parent() -> ToolRegistry {
    Dispatcher::builtin_registry()
}

/// The one directory a call may touch.
pub fn ctx() -> ToolCtx {
    ToolCtx {
        root: PathBuf::from("/work"),
    }
}

/// The arguments the model sends.
pub fn args(description: &str, prompt: &str) -> Value {
    json!({ "description": description, "prompt": prompt })
}

/// A tool with a name and nothing else, so a registry can be made to hold `task` without holding
/// the tool that name belongs to.
pub struct Named(pub &'static str);

impl Tool for Named {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.0.to_string(),
            description: "a stand-in".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, _args: &Value, _ctx: &ToolCtx) -> perch_tools::Result<ToolOutput> {
        Ok(ToolOutput::text(""))
    }
}

/// Whether `names` holds `name`.
pub fn holds(names: &[String], name: &str) -> bool {
    names.iter().any(|held| held == name)
}

/// The words a refused call answered with, which is what a refusal is.
pub fn refused<T>(outcome: std::result::Result<T, perch_tools::Error>) -> String {
    match outcome {
        Err(error) => error.to_string(),
        Ok(_) => panic!("the call was accepted, and it should have been refused"),
    }
}
