//! One entry of the transcript column, and the word a tool card shows for its state.

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
        preview: Option<perch_tools::Preview>,
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
