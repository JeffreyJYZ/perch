//! The walk's state machine: which prompt Up and Down are on, what the box held before, and when
//! an edit ends the walk.
//!
//! The order, the deduplication and the cap are [`perch_store`]'s; this only remembers a position.

use std::path::PathBuf;

use perch_store::history::History;

/// The history the composer is walking, and where it lives.
pub(super) struct Memo {
    pub(super) worktree: Option<PathBuf>,
    pub(super) path: Option<PathBuf>,
    pub(super) history: History,
    walk: Option<Walk>,
}

/// One walk: the step from the newest prompt, the text it put in the box, and the draft.
struct Walk {
    /// Zero is the last prompt sent.
    step: usize,
    /// The text this walk put in the box, so an edit ends it.
    placed: String,
    /// What the box held before the walk started.
    draft: String,
}

/// What one press of Up or Down did.
#[derive(Default)]
pub(super) struct Step {
    /// Whether the press belongs to the history rather than the field.
    pub(super) taken: bool,
    /// The text to put in the box, when the box changes.
    pub(super) text: Option<String>,
}

impl Memo {
    pub(super) const fn new() -> Memo {
        Memo {
            worktree: None,
            path: None,
            history: History::new(),
            walk: None,
        }
    }

    /// Ends a walk whose text the person has edited since.
    pub(super) fn keep_or_end(&mut self, text: &str) {
        if self.walk.as_ref().is_some_and(|walk| walk.placed != text) {
            self.walk = None;
        }
    }

    /// Ends the walk without touching the box: what a send and a load of another worktree do.
    pub(super) fn rewind(&mut self) {
        self.walk = None;
    }

    /// The step `up` or down from where the walk is, starting it when the box is empty.
    pub(super) fn step(&mut self, up: bool, text: &str) -> Step {
        if self.walk.is_none() {
            // Only an empty box starts a walk: anything else is text the person is editing.
            if !up || !text.is_empty() || self.history.is_empty() {
                return Step::default();
            }
            return self.go(0, text.to_owned());
        }
        let step = match self.walk.as_ref() {
            Some(walk) if up => walk.step + 1,
            Some(walk) if walk.step == 0 => return self.draft(),
            Some(walk) => walk.step - 1,
            None => return Step::default(),
        };
        let draft = self
            .walk
            .as_ref()
            .map_or_else(String::new, |walk| walk.draft.clone());
        self.go(step, draft)
    }

    /// Shows the prompt at `step`. Past the oldest it keeps the oldest, which is still the walk's.
    fn go(&mut self, step: usize, draft: String) -> Step {
        let Some(text) = self.history.get(step).map(str::to_owned) else {
            return Step {
                taken: true,
                text: None,
            };
        };
        self.walk = Some(Walk {
            step,
            placed: text.clone(),
            draft,
        });
        Step {
            taken: true,
            text: Some(text),
        }
    }

    /// Down past the newest prompt: the box gets the draft back and the walk ends.
    fn draft(&mut self) -> Step {
        let draft = self.walk.take().map_or_else(String::new, |walk| walk.draft);
        Step {
            taken: true,
            text: Some(draft),
        }
    }
}
