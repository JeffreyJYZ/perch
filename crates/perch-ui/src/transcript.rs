//! The transcript column: the conversation as items, and the view that draws them.
//!
//! [`Transcript`] is plain data — the agent's events in, items out — so the shell can own one per
//! tab and swap between them without reloading anything. [`transcript_view`] is the only drawing
//! code: assistant text goes through rune-kit's markdown view, in streaming mode while the turn
//! is still open, so half-written markup never flashes; a tool call is one card whose status is a
//! word, never a colour alone.

use perch_agent::AgentEvent;
use perch_tools::Preview;
use rune::{Color, El, col, row, text};
use rune_kit::Kit;
use rune_kit::markdown::Markdown;

/// How many lines of a tool's output a collapsed card shows before it says how many are left.
pub const COLLAPSED_LINES: usize = 5;

/// A tool call's state, in the words the column shows for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    /// Called, and not finished yet.
    Running,
    /// Returned without an error.
    Ok,
    /// Returned an error, or refused.
    Failed,
    /// The turn ended before the call did.
    Cancelled,
    /// Waiting for the user to allow it.
    ApprovalNeeded,
}

impl ToolStatus {
    /// The word the card shows. One source for the word, so the status and the text cannot drift.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            ToolStatus::Running => "running",
            ToolStatus::Ok => "done",
            ToolStatus::Failed => "failed",
            ToolStatus::Cancelled => "cancelled",
            ToolStatus::ApprovalNeeded => "approval needed",
        }
    }
}

impl std::fmt::Display for ToolStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.words())
    }
}

/// One entry of the transcript column.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// What the user asked.
    User {
        /// The message as written.
        text: String,
    },
    /// What the model said. The last one streams while a turn is open.
    Assistant {
        /// The reply so far, one string per open turn.
        text: String,
    },
    /// One tool call: its state, its output, and the diff or command an approval shows.
    Tool {
        /// The provider's call id, which is what a result answers.
        id: String,
        /// The tool name, which is the card's one-line summary.
        name: String,
        /// Where the call got to.
        status: ToolStatus,
        /// The result text as the tool returned it, uncollapsed.
        output: String,
        /// The diff or command the approval view needs, when there is one.
        preview: Option<Preview>,
    },
}

/// A tool's output as a collapsed card shows it: the first [`COLLAPSED_LINES`] lines, then one
/// line naming how many are left and how many were shown. Output inside the cap is returned
/// unchanged, so a short result claims nothing.
#[must_use]
pub fn collapsed_output(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= COLLAPSED_LINES {
        return output.trim_end().to_string();
    }
    let hidden = lines.len() - COLLAPSED_LINES;
    format!(
        "{}\n… {hidden} more lines (showing first {COLLAPSED_LINES})",
        lines[..COLLAPSED_LINES].join("\n")
    )
}

/// One session's transcript: the items, plus which assistant item is still growing.
#[derive(Clone, Debug)]
pub struct Transcript {
    items: Vec<Item>,
    /// Index of the assistant item still receiving deltas, if a turn is streaming into one.
    open: Option<usize>,
    /// Draw with the dark palette. The shell sets this from the system; dark is the default.
    dark: bool,
}

impl Default for Transcript {
    fn default() -> Transcript {
        Transcript {
            items: Vec::new(),
            open: None,
            dark: true,
        }
    }
}

impl Transcript {
    /// The items, in the order they were added.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Whether the dark palette is used for this transcript.
    #[must_use]
    pub fn dark(&self) -> bool {
        self.dark
    }

    /// Draws this transcript with the dark palette, or the light one.
    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    /// Adds a user turn and closes whatever was streaming: a new question starts a new answer.
    pub fn push_user(&mut self, text: impl Into<String>) {
        self.items.push(Item::User { text: text.into() });
        self.open = None;
    }

    /// Marks the call `id` as waiting for the user, with the diff or command the approval shows.
    pub fn request_approval(&mut self, id: &str, preview: Preview) {
        if let Some(Item::Tool {
            status,
            preview: slot,
            ..
        }) = self.tool_mut(id)
        {
            *status = ToolStatus::ApprovalNeeded;
            *slot = Some(preview);
        }
    }

    /// Folds one agent event into the transcript.
    pub fn apply(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::TextDelta(delta) => self.push_delta(delta),
            AgentEvent::ToolCall(call) => {
                // A tool card sits between the text before it and the text after it, so the
                // reply must not keep appending to the paragraph above the card.
                self.open = None;
                self.items.push(Item::Tool {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    status: ToolStatus::Running,
                    output: String::new(),
                    preview: None,
                });
            }
            AgentEvent::ToolResult { id, ok, text } => {
                if let Some(Item::Tool { status, output, .. }) = self.tool_mut(id) {
                    *status = if *ok {
                        ToolStatus::Ok
                    } else {
                        ToolStatus::Failed
                    };
                    *output = text.clone();
                }
            }
            AgentEvent::TurnEnd { reason, .. } => self.finish(reason),
        }
    }

    /// Appends to the open assistant item, or opens a new one.
    fn push_delta(&mut self, delta: &str) {
        if let Some(index) = self.open
            && let Some(Item::Assistant { text }) = self.items.get_mut(index)
        {
            text.push_str(delta);
            return;
        }
        self.items.push(Item::Assistant {
            text: delta.to_string(),
        });
        self.open = Some(self.items.len() - 1);
    }

    /// Ends the turn in a definite state: nothing keeps claiming to run, and an error is said.
    fn finish(&mut self, reason: &perch_agent::EndReason) {
        self.open = None;
        match reason {
            perch_agent::EndReason::Done => {}
            perch_agent::EndReason::Cancelled => {
                for item in &mut self.items {
                    if let Item::Tool { status, .. } = item
                        && *status == ToolStatus::Running
                    {
                        *status = ToolStatus::Cancelled;
                    }
                }
            }
            perch_agent::EndReason::Errored(why) => self.items.push(Item::Assistant {
                text: format!("The turn failed: {why}"),
            }),
        }
    }

    /// The tool card whose id is `id`, when there is one.
    fn tool_mut(&mut self, id: &str) -> Option<&mut Item> {
        self.items
            .iter_mut()
            .rev()
            .find(|item| matches!(item, Item::Tool { id: card, .. } if card == id))
    }
}

/// The transcript column: one item after another, each naming what it is.
#[must_use]
pub fn transcript_view(model: &Transcript) -> El {
    let kit = Kit::neutral(model.dark);
    let mut column = col()
        .key("transcript")
        .role("region")
        .label("Transcript")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(20.0)
        .py(16.0)
        .gap(10.0)
        // One selection across the whole column, so a drag copies turns, not one turn.
        .selection_group(true);
    if model.items.is_empty() {
        column = column.child(
            text("No turns yet.")
                .font(kit.ty.body)
                .color(kit.c.text_muted),
        );
    }
    for (index, item) in model.items.iter().enumerate() {
        let streaming = model.open == Some(index);
        column = column.child(item_view(index, item, streaming, &kit));
    }
    column
}

/// One item, keyed by its place in the column so a growing transcript keeps earlier elements.
fn item_view(index: usize, item: &Item, streaming: bool, kit: &Kit) -> El {
    match item {
        Item::User { text: body } => col()
            .key(format!("turn-{index}"))
            .w_full()
            .gap(4.0)
            .px(12.0)
            .py(10.0)
            .radius(kit.radius.md)
            .fill(kit.c.layer_01)
            .child(speaker("You", kit))
            .child(
                text(body.clone())
                    .font(kit.ty.body)
                    .color(kit.c.text_base)
                    .selectable(true),
            ),
        Item::Assistant { text: body } => col()
            .key(format!("turn-{index}"))
            .w_full()
            .gap(4.0)
            .child(speaker("Assistant", kit))
            // Streaming mode parses the tail as what it is about to become, so an unclosed
            // fence or emphasis never flashes its markers; a finished turn renders whole.
            .child(
                Markdown::new(body.clone())
                    .streaming(streaming)
                    .selectable(true)
                    .build(kit)
                    .key("body"),
            ),
        Item::Tool {
            name,
            status,
            output,
            preview,
            ..
        } => tool_card(index, name, *status, output, preview.as_ref(), kit),
    }
}

/// A tool call as one card: what ran, how it ended, and the start of what it returned.
fn tool_card(
    index: usize,
    name: &str,
    status: ToolStatus,
    output: &str,
    preview: Option<&Preview>,
    kit: &Kit,
) -> El {
    let mut card = col()
        .key(format!("tool-{index}"))
        .w_full()
        .gap(6.0)
        .px(12.0)
        .py(10.0)
        .radius(kit.radius.md)
        .fill(kit.c.bg_base)
        .child(
            row()
                .w_full()
                .justify_between()
                .gap(12.0)
                .child(text(name).font(kit.ty.small_medium).color(kit.c.text_base))
                .child(
                    text(status.words())
                        .font(kit.ty.small)
                        .color(status_ink(status, kit)),
                ),
        );
    if let Some(preview) = preview {
        card = card.child(preview_view(preview, kit));
    }
    if !output.is_empty() {
        card = card.child(
            text(collapsed_output(output))
                .font(kit.ty.mono)
                .color(kit.c.text_muted)
                .selectable(true),
        );
    }
    card
}

/// The diff or command an approval shows, whole: the user judges all of it, not five lines.
fn preview_view(preview: &Preview, kit: &Kit) -> El {
    let (label, body) = match preview {
        Preview::Diff(diff) => ("diff", diff),
        Preview::Command(command) => ("command", command),
    };
    col()
        .key("preview")
        .w_full()
        .gap(2.0)
        .child(speaker(label, kit))
        .child(
            text(body.clone())
                .font(kit.ty.mono)
                .color(kit.c.text_muted)
                .selectable(true),
        )
}

/// The small label above a turn's or a card's body.
fn speaker(label: &str, kit: &Kit) -> El {
    text(label)
        .font(kit.ty.small_medium)
        .color(kit.c.text_faint)
}

/// The status word's colour. It reinforces the word; it never carries the meaning alone.
fn status_ink(status: ToolStatus, kit: &Kit) -> Color {
    match status {
        ToolStatus::Running => kit.c.text_muted,
        ToolStatus::Ok => kit.c.success.fg,
        ToolStatus::Failed => kit.c.danger.fg,
        ToolStatus::Cancelled => kit.c.text_faint,
        ToolStatus::ApprovalNeeded => kit.c.warning.fg,
    }
}
