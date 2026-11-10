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

mod words;

pub use words::{COLLAPSED_LINES, cap_line, collapsed_output, expanded_output, summary_line};

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
    /// A system line that belongs to no speaker: a turn that was cancelled, a provider error, a
    /// tool the user refused. The column draws it quieter than a reply, and never as markdown,
    /// because its words are held to be literally true rather than written to be styled.
    Note(String),
    /// How one turn ended: what ran, what failed, and what it cost when the provider said.
    Summary {
        /// Tool calls the turn ran.
        tools: usize,
        /// How many of those returned an error or a refusal.
        failed: usize,
        /// The tokens the provider reported for the turn, when it reported any.
        tokens: Option<u32>,
    },
}

/// One session's transcript: the items, plus which assistant item is still growing.
#[derive(Clone, Debug)]
pub struct Transcript {
    items: Vec<Item>,
    /// Index of the assistant item still receiving deltas, if a turn is streaming into one.
    open: Option<usize>,
    /// Ids of the tool cards showing their whole output; every other card is collapsed.
    expanded: Vec<String>,
    /// Index of the first item of the turn being counted, for its closing summary.
    since: usize,
    /// Draw with the dark palette. The shell sets this from the system; dark is the default.
    dark: bool,
}

impl Default for Transcript {
    fn default() -> Transcript {
        Transcript {
            items: Vec::new(),
            open: None,
            expanded: Vec::new(),
            since: 0,
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
        self.since = self.items.len();
    }

    /// Appends a system line — a cancellation, a provider error, a tool the user refused — in the
    /// column's quieter voice. It closes the open assistant item, because a system line is not
    /// part of a reply.
    pub fn note(&mut self, text: impl Into<String>) {
        self.open = None;
        self.items.push(Item::Note(text.into()));
    }

    /// Flips the tool card `id` between its collapsed lines and its whole output.
    ///
    /// An id that is not a card is ignored, so a click that outlived its card invents nothing.
    pub fn toggle(&mut self, id: &str) {
        if !self
            .items
            .iter()
            .any(|item| matches!(item, Item::Tool { id: card, .. } if card == id))
        {
            return;
        }
        if let Some(index) = self.expanded.iter().position(|card| card == id) {
            self.expanded.remove(index);
        } else {
            self.expanded.push(id.to_string());
        }
    }

    /// Whether the tool card `id` is showing its whole output.
    #[must_use]
    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.iter().any(|card| card == id)
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
            AgentEvent::TurnEnd { reason, usage } => self.finish(reason, *usage),
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

    /// Ends the turn in a definite state: nothing keeps claiming to run, an error is said, and one
    /// line reports what the turn did.
    fn finish(&mut self, reason: &perch_agent::EndReason, usage: perch_agent::Usage) {
        self.open = None;
        let (tools, failed) = self.count_tools();
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
                self.note("The turn was cancelled.");
            }
            perch_agent::EndReason::Errored(why) => {
                self.note(format!("The turn failed: {why}"));
            }
        }
        let tokens = usage.input + usage.output;
        let tokens = (tokens > 0).then_some(tokens);
        // A plain answer with neither a call nor a cost prints nothing: there is nothing to report.
        if tools > 0 || tokens.is_some() {
            self.items.push(Item::Summary {
                tools,
                failed,
                tokens,
            });
        }
        self.since = self.items.len();
    }

    /// How many tool calls this turn ran, and how many of them failed.
    fn count_tools(&self) -> (usize, usize) {
        let mut tools = 0;
        let mut failed = 0;
        for item in &self.items[self.since.min(self.items.len())..] {
            if let Item::Tool { status, .. } = item {
                tools += 1;
                if *status == ToolStatus::Failed {
                    failed += 1;
                }
            }
        }
        (tools, failed)
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
        column = column.child(item_view(index, item, streaming, model, &kit));
    }
    column
}

/// One item, keyed by its place in the column so a growing transcript keeps earlier elements.
fn item_view(index: usize, item: &Item, streaming: bool, model: &Transcript, kit: &Kit) -> El {
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
            id,
            name,
            status,
            output,
            preview,
            ..
        } => tool_card(
            index,
            name,
            *status,
            output,
            preview.as_ref(),
            model.is_expanded(id),
            kit,
        ),
        // A system line is drawn as it is written: one text element, no markdown, in the faint
        // ink a footnote uses, so it never competes with what the model said.
        Item::Note(body) => text(body.clone())
            .key(format!("note-{index}"))
            .w_full()
            .font(kit.ty.small)
            .color(kit.c.text_faint)
            .selectable(true),
        Item::Summary {
            tools,
            failed,
            tokens,
        } => text(summary_line(*tools, *failed, *tokens))
            .key(format!("summary-{index}"))
            .w_full()
            .font(kit.ty.small)
            .color(kit.c.text_faint)
            .selectable(true),
    }
}

/// A tool call as one card: what ran, how it ended, and either the start of what it returned or
/// all of it when the card is expanded.
///
/// The card takes the focus so the window's map can open it: the root's key handler runs before
/// a focused control's own activation rules, which is what lets Enter and Space both reach it.
fn tool_card(
    index: usize,
    name: &str,
    status: ToolStatus,
    output: &str,
    preview: Option<&Preview>,
    expanded: bool,
    kit: &Kit,
) -> El {
    let mut card = col()
        .key(format!("tool-{index}"))
        .role("button")
        .focusable(true)
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
        let shown = if expanded {
            expanded_output(output)
        } else {
            collapsed_output(output)
        };
        card = card.child(
            text(shown)
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
