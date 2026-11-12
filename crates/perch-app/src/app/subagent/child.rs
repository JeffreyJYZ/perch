//! One child turn: an agent on its own thread, and the tools it was allowed to reach.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use perch_agent::cancel::Cancel;
use perch_agent::subagent::Delegate;
use perch_agent::{
    Agent, AgentConfig, AgentEvent, CancelWatch, EndReason, Message, ToolCall, ToolReply, ToolSpec,
    Tools, for_session,
};
use perch_tools::{ToolCtx, ToolRegistry};

use super::Agents;
use super::running::watcher;

/// The most requests one child may make.
///
/// The parent cannot see a child's rounds, so a child bounds itself: this is shorter than the
/// turn's own `MAX_ROUNDS` because a child is a search, and a child left to loop is a hang the
/// parent has no way to notice.
const CHILD_MAX_ROUNDS: usize = 8;

/// How often the wait for the child re-checks cancellation.
///
/// A provider that never answers sends no event, so waiting on the channel alone would make the
/// press felt only once the reply arrived — which is exactly the case a cancel exists for.
const POLL: Duration = Duration::from_millis(25);

/// The delegate `task` is handed, built around the exact tools the tool computed for its child.
pub(super) fn delegate(agents: Agents, tools: ToolRegistry) -> Arc<dyn Delegate> {
    Arc::new(Child {
        agents,
        tools,
        rounds: Arc::new(AtomicUsize::new(0)),
    })
}

struct Child {
    agents: Agents,
    tools: ToolRegistry,
    /// The requests this child made, which is what the result line counts.
    ///
    /// Shared with the tool list's closure rather than passed back, because the answer arrives
    /// through the agent's events and the count through the request that built them.
    rounds: Arc<AtomicUsize>,
}

impl Child {
    /// What the child is told: what it is, then the project's own rules for its worktree.
    fn system(&self, worktree: &Path) -> String {
        let instructions = for_session(worktree);
        if instructions.trim().is_empty() {
            return self.agents.system.clone();
        }
        format!("{}\n\n{instructions}", self.agents.system)
    }

    /// One call, as the child will read it: a tool that failed is content it can act on, never a
    /// panic and never a silent drop.
    fn answer(&self, call: &ToolCall, ctx: &ToolCtx) -> ToolReply {
        let (ok, text) = match self.tools.call(&call.name, &call.args, ctx) {
            Ok(out) => (true, out.text),
            Err(why) => (false, format!("the call failed: {why}")),
        };
        ToolReply {
            message: Message::tool_result(call.id.clone(), text),
            ok,
        }
    }
}

impl Delegate for Child {
    fn run(&self, task: &str, ctx: &ToolCtx, cancel: &Cancel) -> Result<String, String> {
        self.rounds.store(0, Ordering::SeqCst);
        let counted = Arc::clone(&self.rounds);
        let specs = agent_specs(self.tools.specs());
        // The list is fixed for the child's whole turn; the closure is here only to count. A round
        // in this codebase is one request, and the request is the moment the list is read.
        let tools = Tools::live(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            specs.clone()
        });
        let (handle, events) = Agent::spawn(AgentConfig {
            endpoint: self.agents.endpoint.clone(),
            model: self.agents.model.clone(),
            system: self.system(&ctx.root),
            tools,
        });
        handle.prompt(task.to_string());
        let generation = cancel.begin();
        let watch = watcher(&ctx.root);
        let mut answer = String::new();
        loop {
            if cancel.stopped(generation) || watch.as_ref().is_some_and(CancelWatch::cancelled) {
                handle.cancel();
                return Err("cancelled".to_string());
            }
            match events.recv_timeout(POLL) {
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("the child stopped before it answered".to_string());
                }
                Ok(AgentEvent::TextDelta(text)) => answer.push_str(&text),
                Ok(AgentEvent::ToolCall(call)) => {
                    if !handle.reply(self.answer(&call, ctx)) {
                        return Err(
                            "the child stopped before its tool result went back".to_string()
                        );
                    }
                }
                Ok(AgentEvent::TurnEnd { reason, .. }) => {
                    return match reason {
                        EndReason::Done => Ok(answer),
                        EndReason::Cancelled => {
                            handle.cancel();
                            Err("cancelled".to_string())
                        }
                        EndReason::Errored(why) => Err(why),
                    };
                }
                Ok(_) => {}
            }
            if self.rounds.load(Ordering::SeqCst) > CHILD_MAX_ROUNDS {
                handle.cancel();
                return Err(format!(
                    "the child reached its {CHILD_MAX_ROUNDS}-round limit without answering"
                ));
            }
        }
    }

    fn rounds(&self) -> Option<usize> {
        let rounds = self.rounds.load(Ordering::SeqCst);
        (rounds > 0).then_some(rounds)
    }
}

/// The child's tools as the wire's: the same conversion the session's own list goes through.
fn agent_specs(specs: Vec<perch_tools::ToolSpec>) -> Vec<ToolSpec> {
    specs
        .into_iter()
        .map(|spec| ToolSpec {
            name: spec.name,
            description: spec.description,
            parameters: spec.parameters,
        })
        .collect()
}
