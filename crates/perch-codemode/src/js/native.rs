//! The three host functions a program sees: `print`, `tools.call` and `tools.list`.

use boa_engine::{
    Context, JsNativeError, JsResult, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, property::Attribute,
};
use perch_tools::{Preview, ToolOutput};
use serde_json::{Map, Value, json};

use super::state::{self, State};
use crate::diag::Stop;
use crate::host::refusal;

/// Registers `tools` and `print` on a fresh context.
pub(super) fn install(context: &mut Context) -> JsResult<()> {
    let tools = {
        let mut builder = ObjectInitializer::new(context);
        builder
            .function(NativeFunction::from_fn_ptr(call), js_string!("call"), 2)
            .function(NativeFunction::from_fn_ptr(list), js_string!("list"), 0);
        builder.build()
    };
    context.register_global_property(
        js_string!("tools"),
        tools,
        Attribute::WRITABLE | Attribute::ENUMERABLE | Attribute::CONFIGURABLE,
    )?;
    context.register_global_callable(js_string!("print"), 1, NativeFunction::from_fn_ptr(print))
}

/// `print(...values)` writes one line and charges it against the output budget.
fn print(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let mut line = String::new();
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            line.push(' ');
        }
        line.push_str(&arg.to_string(context)?.to_std_string_escaped());
    }
    ask(|state| {
        state.budget.poll()?;
        let bytes = state.out.line(&line);
        state.budget.charge_output(bytes)
    })?;
    Ok(JsValue::undefined())
}

/// `tools.call(name, args)` runs a tool and answers `{ text }`, throwing on a failure.
fn call(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(name) = args.first().and_then(JsValue::as_string) else {
        return Err(JsNativeError::typ()
            .with_message("tools.call needs a tool name as a string")
            .into());
    };
    let name = name.to_std_string_escaped();
    let arguments = match args.get(1) {
        None => Value::Null,
        Some(given) if given.is_undefined() => Value::Null,
        Some(given) => match given.as_string() {
            Some(text) => {
                let text = text.to_std_string_escaped();
                serde_json::from_str(&text).map_err(|error| {
                    JsNativeError::typ()
                        .with_message(format!("the arguments for `{name}` are not JSON: {error}"))
                })?
            }
            None => given.to_json(context)?.unwrap_or(Value::Null),
        },
    };
    // The approval boundary: refused before it is charged or run, and thrown as an ordinary
    // error, so a program learns why and a person still approves the call made directly.
    if state::with(|state| state.host.gated(&name)).unwrap_or(false) {
        return Err(JsNativeError::error().with_message(refusal(&name)).into());
    }
    let answer = ask(|state| {
        state.budget.poll()?;
        state.budget.charge_call()?;
        Ok(state.host.call(&name, &arguments, &state.ctx))
    })?;
    match answer {
        Ok(output) => JsValue::from_json(&payload(&output), context),
        Err(error) => Err(JsNativeError::error()
            .with_message(error.to_string())
            .into()),
    }
}

/// `tools.list()` answers `[{ name, description }]` for every registered tool.
fn list(_this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let items: Vec<Value> = state::with(|state| {
        state
            .host
            .specs()
            .into_iter()
            .map(|spec| json!({ "name": spec.name, "description": spec.description }))
            .collect()
    })
    .unwrap_or_default();
    JsValue::from_json(&Value::Array(items), context)
}

/// Runs one host step under the budgets, turning a stop into a thrown error **and** a recorded
/// stop, so catching the error cannot buy the program any more budget.
fn ask<R>(f: impl FnOnce(&mut State) -> Result<R, Stop>) -> JsResult<R> {
    let outcome = state::with(|state| {
        let outcome = f(state);
        if let Err(stop) = &outcome {
            state.stop = Some(stop.clone());
        }
        outcome
    });
    match outcome {
        Some(Ok(value)) => Ok(value),
        Some(Err(stop)) => Err(JsNativeError::error().with_message(stop.words()).into()),
        None => Err(JsNativeError::error()
            .with_message("no program is running")
            .into()),
    }
}

/// A tool's output as the object a program reads: `{ text }`, plus the preview when there is one.
fn payload(output: &ToolOutput) -> Value {
    let mut map = Map::new();
    map.insert(String::from("text"), Value::String(output.text.clone()));
    if let Some(preview) = &output.preview {
        let (kind, value) = match preview {
            Preview::Diff(text) => ("diff", text),
            Preview::Command(command) => ("command", command),
        };
        map.insert(
            String::from("preview"),
            json!({ "kind": kind, "value": value }),
        );
    }
    Value::Object(map)
}
