//! One live turn against a real provider, for a model named on the command line.
//!
//! The unit tests replay recorded SSE bodies; this runs the same [`Agent`] against
//! OpenRouter, with the six built-ins confined to a fresh temporary directory and an
//! interactor that allows everything. The prompt forces at least two rounds: a write, a
//! read, an append, and a sentence that can only be written after the tool results came
//! back. Anything printed is a name, a count or a verdict — never the key, never a
//! request header.
//!
//! The key arrives in `OPENROUTER_API_KEY`, never on the command line, so it cannot be
//! read back out of the process table. Exit code 0 means the loop genuinely ran.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use perch_agent::{
    Agent, AgentConfig, AgentEvent, AgentHandle, Answer, Decision, Dispatcher, EndReason,
    Interactor, SecretString, ToolReply, Tools,
};
use perch_tools::{Preview, ToolCtx, ToolRegistry};

/// The API root `/chat/completions` is appended to.
const BASE_URL: &str = "https://openrouter.ai/api/v1";

/// How long one event may take before the probe gives up. The provider client's own
/// deadline is 120 s per request, so this only has to outlast a whole turn.
const EVENT_TIMEOUT: Duration = Duration::from_secs(240);

/// How long to wait for the rest of a round's calls: the turn emits them back to back
/// before it waits for any reply, so they are already queued when the first one is read.
const CALL_DRAIN: Duration = Duration::from_millis(250);

/// Longest final answer printed, so one chatty model does not bury the report.
const ANSWER_LIMIT: usize = 1200;

/// The system message: the tools do the work, not the words.
const SYSTEM: &str = "You are a coding agent working in the current directory. \
Use the tools to change files; do not describe a change you have not made. \
When the task is done, state the result in one short paragraph.";

/// A prompt that cannot be satisfied in one round: the fourth line depends on what the
/// file held after the write, and the line count depends on the append.
const PROMPT: &str = "Create notes.txt with three short lines about a terminal, \
then read it back and append a fourth line, then tell me the file's final line count.";

fn main() {
    std::process::exit(run());
}

/// The probe itself, returning the exit status.
fn run() -> i32 {
    let Some(model) = std::env::args().nth(1) else {
        eprintln!("usage: live_probe <model>");
        return 2;
    };
    let Ok(key) = std::env::var("OPENROUTER_API_KEY") else {
        eprintln!("live_probe: OPENROUTER_API_KEY is not set");
        return 2;
    };
    if key.trim().is_empty() {
        eprintln!("live_probe: OPENROUTER_API_KEY is empty");
        return 2;
    }

    let Ok(scratch) = tempfile::tempdir() else {
        eprintln!("live_probe: cannot create a temporary directory");
        return 2;
    };
    let root = scratch.path().to_path_buf();
    let ctx = ToolCtx { root: root.clone() };
    let registry = Dispatcher::builtin_registry();

    let (handle, events) = Agent::spawn(AgentConfig {
        base_url: BASE_URL.to_string(),
        api_key: SecretString::from(key),
        model: model.clone(),
        system: SYSTEM.to_string(),
        // The registry's spec and the wire spec are separate types; the app maps them the
        // same way, so the probe offers exactly what a session would.
        tools: Tools::fixed(
            registry
                .specs()
                .into_iter()
                .map(|spec| perch_agent::ToolSpec {
                    name: spec.name,
                    description: spec.description,
                    parameters: spec.parameters,
                })
                .collect(),
        ),
    });
    handle.prompt(PROMPT.to_string());

    // The prompt is the first request; each round of calls buys the next one.
    let mut run = Run {
        requests: 1,
        ..Run::default()
    };
    let end = drive(&handle, &events, &registry, &ctx, &mut run);

    let path = root.join("notes.txt");
    let exists = path.is_file();
    let body = std::fs::read_to_string(&path).unwrap_or_default();
    let lines = body.lines().count();

    let looped = run.requests >= 2;
    let passed = end == EndReason::Done && looped && !run.names.is_empty() && exists;

    println!("model: {model}");
    println!("end: {end:?}");
    println!(
        "requests made: {} (the prompt plus {} round(s) of calls)",
        run.requests,
        run.rounds.len()
    );
    println!(
        "rounds, in request order: {}",
        if run.rounds.is_empty() {
            "(none)".to_string()
        } else {
            run.rounds.join(" | ")
        }
    );
    println!(
        "tool calls in order: {}",
        if run.names.is_empty() {
            "(none)".to_string()
        } else {
            run.names.join(", ")
        }
    );
    println!(
        "tool-call ids seen: {}",
        if run.ids.is_empty() {
            "(none)".to_string()
        } else {
            run.ids.join(", ")
        }
    );
    println!(
        "approvals recorded: {} ({})",
        run.interactor.approvals.len(),
        if run.interactor.approvals.is_empty() {
            "none".to_string()
        } else {
            run.interactor.approvals.join("; ")
        }
    );
    println!("questions recorded: {}", run.interactor.questions.len());
    println!(
        "tokens reported: input {}, output {}",
        run.usage.0, run.usage.1
    );
    println!("tool result fed a second request: {looped}");
    println!("final assistant text: {}", clipped(&run.final_text));
    println!(
        "notes.txt exists: {exists}{}",
        if exists { "" } else { " (nothing was written)" }
    );
    println!("notes.txt lines: {lines}");
    println!(
        "verdict: {}",
        if passed {
            "PASS".to_string()
        } else {
            format!("FAIL ({})", why(end, looped, exists))
        }
    );
    if passed { 0 } else { 1 }
}

/// Why a finished run is not a pass, in the words of the thing that is missing.
fn why(end: EndReason, looped: bool, exists: bool) -> String {
    if end != EndReason::Done {
        return "the turn did not finish".to_string();
    }
    if !looped {
        return "the turn made one request, so no tool result was sent back".to_string();
    }
    if !exists {
        return "notes.txt was never created".to_string();
    }
    "no tool was called".to_string()
}

/// Read the turn's events, running every round of calls it asks for, until it ends.
fn drive(
    handle: &AgentHandle,
    events: &Receiver<AgentEvent>,
    registry: &ToolRegistry,
    ctx: &ToolCtx,
    run: &mut Run,
) -> EndReason {
    let mut dispatcher = Dispatcher::new();
    // An event already read while gathering a round's calls, to be handled next turn.
    let mut stash: Option<AgentEvent> = None;
    loop {
        let event = match stash.take() {
            Some(event) => event,
            None => match events.recv_timeout(EVENT_TIMEOUT) {
                Ok(event) => event,
                Err(_) => {
                    return EndReason::Errored(format!(
                        "no event arrived within {} s",
                        EVENT_TIMEOUT.as_secs()
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
                    match events.recv_timeout(CALL_DRAIN) {
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
struct Run {
    /// One for the prompt plus one per round that asked for tools, so a run that never
    /// reaches two never fed a tool result back.
    requests: usize,
    /// Each round's calls, in request order, so the count above is checkable.
    rounds: Vec<String>,
    names: Vec<String>,
    ids: Vec<String>,
    text: String,
    final_text: String,
    usage: (u32, u32),
    interactor: Scripted,
}

/// Allows every call, and remembers what it was asked about.
#[derive(Default)]
struct Scripted {
    approvals: Vec<String>,
    questions: Vec<String>,
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

/// `text` on one line, cut to [`ANSWER_LIMIT`], so the report stays readable.
fn clipped(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return "(no text)".to_string();
    }
    if flat.chars().count() <= ANSWER_LIMIT {
        return flat;
    }
    let head: String = flat.chars().take(ANSWER_LIMIT).collect();
    format!("{head}… [{} chars total]", flat.chars().count())
}
