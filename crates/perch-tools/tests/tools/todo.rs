//! The plan: one list per worktree, replaced whole, with one step in progress at a time.

use perch_tools::{MAX_TODOS, Todo, TodoStatus, ToolCtx, ToolRegistry};
use serde_json::{Value, json};

use crate::support::root_of;

/// Writes `todos` and answers the summary line the tool said back.
pub(crate) fn write(registry: &ToolRegistry, ctx: &ToolCtx, todos: Value) -> String {
    registry
        .call("todowrite", &json!({"todos": todos}), ctx)
        .unwrap()
        .text
}

/// Reads the list back as the JSON the tool returned.
pub(crate) fn read(registry: &ToolRegistry, ctx: &ToolCtx) -> Vec<Todo> {
    let text = registry.call("todoread", &json!({}), ctx).unwrap().text;
    if text == "no todos" {
        return Vec::new();
    }
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{text} is not the list: {error}"))
}

pub(crate) fn item(content: &str, status: &str) -> Value {
    json!({"content": content, "status": status})
}

#[test]
fn writing_a_list_replaces_it_and_sending_fewer_items_removes_the_rest() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let summary = write(
        &registry,
        &ctx,
        json!([
            item("read the spec", "completed"),
            item("write the tool", "in_progress"),
            item("draw the panel", "pending"),
        ]),
    );
    assert_eq!(summary, "3 todos: 1 done, 1 in progress, 1 pending");
    let first = read(&registry, &ctx);
    assert_eq!(first.len(), 3, "{first:?}");
    assert_eq!(first[0].content, "read the spec");
    assert_eq!(first[1].status, TodoStatus::InProgress);

    // The same tool again with fewer items: the list is what was sent, not what was there.
    write(
        &registry,
        &ctx,
        json!([
            item("write the tool", "completed"),
            item("draw the panel", "pending")
        ]),
    );
    let second = read(&registry, &ctx);
    assert_eq!(second.len(), 2, "{second:?}");
    assert_eq!(
        second
            .iter()
            .map(|todo| todo.content.as_str())
            .collect::<Vec<_>>(),
        ["write the tool", "draw the panel"]
    );
    assert!(
        !second.iter().any(|todo| todo.content == "read the spec"),
        "a dropped item survived the replace: {second:?}"
    );
}

#[test]
fn an_empty_array_clears_the_list() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    write(&registry, &ctx, json!([item("something", "pending")]));
    assert_eq!(read(&registry, &ctx).len(), 1);

    let summary = write(&registry, &ctx, json!([]));
    assert_eq!(summary, "0 todos: 0 done, 0 in progress, 0 pending");
    assert!(read(&registry, &ctx).is_empty(), "the list was not cleared");
}

#[test]
fn two_in_progress_items_are_refused_and_the_list_is_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    write(
        &registry,
        &ctx,
        json!([item("first", "in_progress"), item("second", "pending")]),
    );

    let refused = registry
        .call(
            "todowrite",
            &json!({"todos": [
                item("first", "in_progress"),
                item("second", "in_progress"),
            ]}),
            &ctx,
        )
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("in_progress") && refused.contains("one"),
        "the refusal does not name the rule: {refused}"
    );

    let kept = read(&registry, &ctx);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert_eq!(
        kept.iter()
            .filter(|todo| todo.status == TodoStatus::InProgress)
            .count(),
        1,
        "the refused write changed the stored list: {kept:?}"
    );
}

#[test]
fn the_thirty_third_todo_is_refused_and_the_refusal_names_the_cap() {
    let root = tempfile::tempdir().unwrap();
    let ctx = root_of(&root);
    let registry = ToolRegistry::builtin();

    let at_cap: Vec<Value> = (0..MAX_TODOS)
        .map(|index| item(&format!("step {index}"), "pending"))
        .collect();
    write(&registry, &ctx, Value::Array(at_cap));
    assert_eq!(read(&registry, &ctx).len(), MAX_TODOS);

    let over: Vec<Value> = (0..=MAX_TODOS)
        .map(|index| item(&format!("step {index}"), "pending"))
        .collect();
    let refused = registry
        .call("todowrite", &json!({"todos": over}), &ctx)
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains(&MAX_TODOS.to_string()),
        "the refusal does not name the cap of {MAX_TODOS}: {refused}"
    );
    assert_eq!(
        read(&registry, &ctx).len(),
        MAX_TODOS,
        "a refused list beyond the cap reached the table"
    );
}
