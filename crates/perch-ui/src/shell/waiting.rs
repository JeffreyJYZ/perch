//! What a session is waiting on, as the key map and the answer card need it.
//!
//! Kept apart from the [`Live`](super::live::Live) seam so the words a wait is written in have one
//! home: the key map asks what a press may do with it, and the card says what it expects.

/// What the active session is waiting on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Waiting {
    /// Nothing: a turn is running, or none has started.
    #[default]
    Nothing,
    /// A call that needs the user's decision.
    Approval,
    /// The model's question, with `options` choices, several of them when `multiple`.
    Question {
        /// How many options the question offered.
        options: usize,
        /// Whether more than one option may be picked.
        multiple: bool,
    },
}

impl Waiting {
    /// The words the window says while this is what it waits on.
    pub fn words(self) -> &'static str {
        match self {
            Waiting::Nothing => "",
            Waiting::Approval => "approval needed",
            Waiting::Question { .. } => "answer needed",
        }
    }

    /// Whether the question takes several options at once.
    pub fn multiple(self) -> bool {
        matches!(self, Waiting::Question { multiple: true, .. })
    }
}
