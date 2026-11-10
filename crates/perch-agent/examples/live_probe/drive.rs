//! Reading the turn's events and answering every round of calls it asks for.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use perch_agent::{
    AgentEvent, AgentHandle, Answer, Decision, Dispatcher, EndReason, Interactor, ToolReply,
};
use perch_tools::{Preview, ToolCtx, ToolRegistry};

/// Read the turn's events, running every round of calls it asks for, until it ends.
pub(super) fn drive(
    handle: &AgentHandle,
    events: &Receiver<AgentEvent>,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    run: &mut Run,
    event_timeout: Duration,
    call_drain: Duration,
) -> EndReason {
    let mut dispatcher = Dispatcher::new();
    // An event already read while gathering a round's calls, to be handled next turn.
    let mut stash: Option<AgentEvent> = None;
    loop {
        let event = match stash.take() {
            Some(event) => event,
            None => match events.recv_timeout(event_timeout) {
                Ok(event) => event,
                Err(_) => {
                    return EndReason::Errored(format!(
                        "no event arrived within {} s",
                        event_timeout.as_secs()
                    ));
                }
            },
        };
        match event {
            AgentEvent::TextDelta(delta) => run.text.push_str(&delta),
            AgentEvent::ToolCall(call) => {
                let mut batch = vec![call];
                // A round emits all of its calls before it waits for the first reply, so
                // the rest of them are already queued and one drain collects the round.
                loop {
                    match events.recv_timeout(call_drain) {
                        Ok(AgentEvent::ToolCall(next)) => batch.push(next),
                        Ok(other) => {
                            stash = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                // The prompt was the first request; each round of calls buys the next one.
                run.requests += 1;
                // Whatever came before the calls was a preamble, not the answer.
                run.text.clear();
                for call in &batch {
                    run.names.push(call.name.clone());
                    run.ids.push(call.id.clone());
                }
                run.rounds.push(
                    batch
                        .iter()
                        .map(|call| call.name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                let replies: Vec<ToolReply> =
                    dispatcher.run(&batch, registry, ctx, &mut run.interactor);
                for reply in replies {
                    handle.reply(reply);
                }
            }
            // The runner emits these, not the agent, so there is nothing to see here.
            AgentEvent::ToolResult { .. } => {}
            AgentEvent::TurnEnd { reason, usage } => {
                run.usage = (usage.input, usage.output);
                run.final_text = std::mem::take(&mut run.text);
                return reason;
            }
        }
    }
}

/// Everything one run saw, filled in as its events arrive.
#[derive(Default)]
pub(super) struct Run {
    /// One for the prompt plus one per round that asked for tools, so a run that never
    /// reaches two never fed a tool result back.
    pub(super) requests: usize,
    /// Each round's calls, in request order, so the count above is checkable.
    pub(super) rounds: Vec<String>,
    pub(super) names: Vec<String>,
    pub(super) ids: Vec<String>,
    pub(super) text: String,
    pub(super) final_text: String,
    pub(super) usage: (u32, u32),
    pub(super) interactor: Scripted,
}

/// Allows every call, and remembers what it was asked about.
#[derive(Default)]
pub(super) struct Scripted {
    pub(super) approvals: Vec<String>,
    pub(super) questions: Vec<String>,
}

impl Interactor for Scripted {
    fn approve(&mut self, name: &str, preview: Option<&Preview>) -> Decision {
        let what = match preview {
            Some(Preview::Diff(diff)) => format!("{name} ({} diff lines)", diff.lines().count()),
            Some(Preview::Command(command)) => format!("{name}: {command}"),
            None => name.to_string(),
        };
        self.approvals.push(what);
        Decision::Allow
    }

    /// A scripted run answers the first option, and says so when there is nothing to pick.
    fn ask(&mut self, prompt: &str, options: &[String], _multiple: bool) -> Answer {
        self.questions.push(prompt.to_string());
        match options.first() {
            Some(first) => Answer::Text(first.clone()),
            None => Answer::Text("no answer".to_string()),
        }
    }
}
