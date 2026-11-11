//! Running one program: a boa context with `tools` and `print`, and the outcome as data.

mod native;
mod state;

use std::sync::Arc;

use boa_engine::{Context, JsError, JsValue, Script, Source};
use perch_tools::ToolCtx;

use crate::budget::Budget;
use crate::diag::{Stop, render};
use crate::host::ToolHost;
use crate::output::Out;
use state::State;

/// How one execution ended.
pub(super) enum Outcome {
    /// It finished; whatever it printed or returned is in [`Finished::out`].
    Ok,
    /// A budget, or the turn's cancel, stopped it.
    Stopped(Stop),
    /// It threw and nothing caught it.
    Threw { message: String, stack: String },
    /// It never ran, because it does not parse.
    Unparsed { message: String, diagnostic: String },
}

/// What one execution leaves behind.
pub(super) struct Finished {
    pub outcome: Outcome,
    pub out: Out,
}

/// Parses and runs `program` with `tools` and `print` bound, under `budget`.
pub(super) fn run(
    program: &str,
    host: Arc<dyn ToolHost>,
    ctx: ToolCtx,
    budget: Budget,
    out: Out,
) -> Finished {
    let steps = budget.limits().steps;
    let previous = state::enter(State {
        host,
        ctx,
        budget,
        out,
        stop: None,
    });

    let mut context = Context::default();
    context.runtime_limits_mut().set_loop_iteration_limit(steps);
    let outcome = match native::install(&mut context) {
        Ok(()) => evaluate(program, &mut context),
        Err(error) => Outcome::Threw {
            message: error.to_string(),
            stack: String::new(),
        },
    };

    let mut state = state::leave(previous).expect("this call entered its own state");
    // A recorded stop wins over the outcome: a program that caught the budget's error still
    // spent the budget, and it must not be able to report success.
    let outcome = match state.stop.take() {
        Some(stop) => Outcome::Stopped(stop),
        None => outcome,
    };
    Finished {
        outcome,
        out: state.out,
    }
}

/// Parses, evaluates, and reports what the program's last expression answered.
fn evaluate(program: &str, context: &mut Context) -> Outcome {
    let script = match Script::parse(Source::from_bytes(program), None, context) {
        Ok(script) => script,
        Err(error) => return unparsed(program, &error),
    };
    match script.evaluate(context) {
        Ok(value) => {
            let text = completion(&value, context);
            if text.is_empty() {
                return Outcome::Ok;
            }
            match state::with(|state| {
                let bytes = state.out.value(&text);
                state.budget.charge_output(bytes)
            }) {
                Some(Err(stop)) => Outcome::Stopped(stop),
                _ => Outcome::Ok,
            }
        }
        Err(error) => thrown(&error),
    }
}

/// The program's completion value as text, and nothing at all for `undefined`.
fn completion(value: &JsValue, context: &mut Context) -> String {
    if value.is_undefined() {
        return String::new();
    }
    match value.to_json(context) {
        Ok(Some(serde_json::Value::String(text))) => text,
        Ok(Some(other)) => other.to_string(),
        _ => value
            .to_string(context)
            .map(|text| text.to_std_string_escaped())
            .unwrap_or_default(),
    }
}

/// An uncaught throw: the message, and boa's stack when it has one.
fn thrown(error: &JsError) -> Outcome {
    let text = error.to_string();
    // boa counts loop iterations instead of watching a clock, so this is how a spin ends. The
    // stop is built here because no host function saw it happen.
    if text.contains("maximum number of iteration loops")
        && let Some(stop) = state::with(|state| {
            let steps = state.budget.limits().steps;
            state
                .stop
                .clone()
                .unwrap_or_else(|| state.budget.steps_hit(steps))
        })
    {
        return Outcome::Stopped(stop);
    }
    let message = text
        .lines()
        .next()
        .unwrap_or("the program threw")
        .to_string();
    Outcome::Threw {
        message,
        stack: text,
    }
}

/// A program that does not parse: its line and column, drawn against the source.
fn unparsed(program: &str, error: &JsError) -> Outcome {
    let text = error.to_string();
    let (line, column) = place(&text).unwrap_or((1, 1));
    let first = text.lines().next().unwrap_or("the program does not parse");
    Outcome::Unparsed {
        message: format!("the program does not parse at line {line}, column {column}"),
        diagnostic: render(program, line, column, &without_place(first)),
    }
}

/// The line and column boa's parser put in its message.
fn place(text: &str) -> Option<(usize, usize)> {
    let at = text.rfind("at line ")?;
    let rest = text.get(at + "at line ".len()..)?;
    let (line, tail) = rest.split_once(", col ")?;
    let column: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    Some((line.trim().parse().ok()?, column.parse().ok()?))
}

/// The same message without the position it already states, which the caret states instead.
fn without_place(text: &str) -> String {
    match text.rfind(" at line ") {
        Some(at) => text[..at].to_string(),
        None => text.to_string(),
    }
}
