//! Which files the last finished turn named, read from the transcript's own cards.
//!
//! A turn's record is the items between one closing summary and the next. A card's `target` is
//! what the call acted on as its arguments name it, so the files a turn *wrote* are the targets of
//! its writing tools and nothing else: a `read` changed nothing, and a shell call's target is its
//! command rather than a file.
//!
//! Nothing here reads the worktree, so the answer is the same for a clean tree and a dirty one.
//! Whether a named file really changed is the diff's question, not this one.

use super::{Item, Transcript};

/// The tools whose target is the path they wrote.
///
/// `patch` is deliberately absent: its argument is a unified diff, so the transcript holds no path
/// for the files it wrote. Trusting a second source of truth for that would be worse than leaving
/// it out, and `/diff` says which files came from where.
const WRITING: [&str; 2] = ["write", "edit"];

impl Transcript {
    /// The paths the last finished turn's writing tools named, in the order they were named.
    ///
    /// `None` is no finished turn in this transcript at all — nothing has ended yet, or the column
    /// was cleared — which is a different answer from a turn that ended and changed nothing.
    #[must_use]
    pub fn last_turn_files(&self) -> Option<Vec<String>> {
        let close = self
            .items
            .iter()
            .rposition(|item| matches!(item, Item::Summary { .. }))?;
        let open = self.items[..close]
            .iter()
            .rposition(|item| matches!(item, Item::Summary { .. }))
            .map_or(0, |index| index + 1);
        let mut files: Vec<String> = Vec::new();
        for item in &self.items[open..close] {
            let Item::Tool {
                name,
                target: Some(target),
                ..
            } = item
            else {
                continue;
            };
            if !WRITING.contains(&name.as_str()) || files.contains(target) {
                continue;
            }
            files.push(target.clone());
        }
        Some(files)
    }
}

#[cfg(test)]
mod tests {
    use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};
    use serde_json::{Value, json};

    use super::Transcript;

    /// A path argument, the way a writing tool names it.
    fn path(path: &str) -> Value {
        json!({ "path": path })
    }

    /// One tool call, finished, on `model`.
    fn call(model: &mut Transcript, index: usize, name: &str, args: Value) {
        let id = format!("call-{index}");
        model.apply(&AgentEvent::ToolCall(ToolCall {
            id: id.clone(),
            name: name.to_string(),
            args,
        }));
        model.apply(&AgentEvent::ToolResult {
            id,
            ok: true,
            text: String::new(),
        });
    }

    /// A finished turn of `(name, arguments)` calls, on `model`.
    fn turn(model: &mut Transcript, calls: &[(&str, Value)]) {
        for (index, (name, args)) in calls.iter().enumerate() {
            call(model, index, name, args.clone());
        }
        model.apply(&AgentEvent::TurnEnd {
            reason: EndReason::Done,
            usage: Usage::default(),
        });
    }

    #[test]
    fn the_files_are_the_writing_targets_of_the_last_turn_only() {
        let mut model = Transcript::default();
        turn(&mut model, &[("write", path("past.txt"))]);
        turn(
            &mut model,
            &[
                ("write", path("a.txt")),
                ("edit", path("b.rs")),
                ("read", path("c.txt")),
                ("bash", json!({ "command": "cargo test" })),
            ],
        );

        assert_eq!(
            model.last_turn_files(),
            Some(vec!["a.txt".to_string(), "b.rs".to_string()]),
            "the turn that just ended, without the read or the shell call"
        );
    }

    #[test]
    fn a_file_named_twice_in_one_turn_is_named_once() {
        let mut model = Transcript::default();
        turn(
            &mut model,
            &[("edit", path("a.txt")), ("write", path("a.txt"))],
        );

        assert_eq!(model.last_turn_files(), Some(vec!["a.txt".to_string()]));
    }

    #[test]
    fn a_turn_that_wrote_nothing_names_no_file() {
        let mut model = Transcript::default();
        turn(
            &mut model,
            &[("read", path("a.txt")), ("glob", path("*.rs"))],
        );

        assert_eq!(
            model.last_turn_files(),
            Some(Vec::new()),
            "the turn ended, and it wrote nothing"
        );
    }

    #[test]
    fn no_finished_turn_is_none_rather_than_empty() {
        let mut model = Transcript::default();
        assert_eq!(model.last_turn_files(), None);

        model.push_user("what changed?");
        assert_eq!(
            model.last_turn_files(),
            None,
            "a turn still running has no record to read"
        );
    }
}
