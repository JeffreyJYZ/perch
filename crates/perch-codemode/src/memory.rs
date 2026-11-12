//! A host that answers from a table, for tests and for embedding without a registry.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use perch_tools::{Error, Result, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::host::ToolHost;

/// Answers each tool with a fixed text, remembers every call, and can mark one gated.
#[derive(Debug, Default)]
pub struct MemoryHost {
    answers: BTreeMap<String, String>,
    gated: BTreeSet<String>,
    calls: Mutex<Vec<(String, Value)>>,
}

impl MemoryHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a tool that always answers `text`.
    pub fn answering(mut self, name: &str, text: &str) -> Self {
        self.answers.insert(name.to_string(), text.to_string());
        self
    }

    /// Registers a tool that asks a person first, so a program must be refused it.
    pub fn gated(mut self, name: &str) -> Self {
        self.answers.entry(name.to_string()).or_default();
        self.gated.insert(name.to_string());
        self
    }

    /// Every call this host was asked to run, in order, with the arguments it was given.
    pub fn calls(&self) -> Vec<(String, Value)> {
        self.log().clone()
    }

    /// How many calls it was asked to run.
    pub fn count(&self) -> usize {
        self.log().len()
    }

    fn log(&self) -> std::sync::MutexGuard<'_, Vec<(String, Value)>> {
        self.calls
            .lock()
            .expect("the call log is only ever held for a push")
    }
}

impl ToolHost for MemoryHost {
    fn call(&self, name: &str, args: &Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        self.log().push((name.to_string(), args.clone()));
        match self.answers.get(name) {
            Some(text) => Ok(ToolOutput::text(text.clone())),
            None => Err(Error::UnknownTool(name.to_string())),
        }
    }

    fn specs(&self) -> Vec<ToolSpec> {
        self.answers
            .keys()
            .map(|name| ToolSpec {
                name: name.clone(),
                description: format!("the test host's `{name}`"),
                parameters: json!({ "type": "object" }),
            })
            .collect()
    }

    fn gated(&self, name: &str) -> bool {
        self.gated.contains(name)
    }
}
