//! The answer to a waiting question: the options picked, the text field, and the keys in words.
//!
//! A question may take one option or several, and may always be answered in the person's own
//! words instead. The state lives in one place so the card that shows it and the keys that change
//! it read the same values, and so a new question always starts from nothing.
//!
//! Every refusal is a sentence rather than a silent no-op: Enter with nothing picked, and Enter
//! over an empty field, both keep the question waiting and say what is missing. Sending an empty
//! pick would read to the model as "the user chose nothing", which it would then guess at.

use rune::TextEdit;

use super::live::{Live, Waiting};
use super::state::Shell;

/// What a waiting question's answer is made of.
#[derive(Default)]
pub(super) struct Answering {
    /// The options picked, in the order they were picked.
    pub(super) picked: Vec<usize>,
    /// Whether the text field is open.
    pub(super) typing: bool,
    /// What the text field holds.
    pub(super) draft: TextEdit,
    /// Why the last confirm answered nothing, in words.
    pub(super) note: Option<String>,
}

impl<L: Live> Shell<L> {
    /// The option a single-choice question has picked, when exactly one is.
    #[must_use]
    pub fn picked(&self) -> Option<usize> {
        match self.answer.picked.as_slice() {
            [index] => Some(*index),
            _ => None,
        }
    }

    /// Every option picked so far, in the order they were picked.
    #[must_use]
    pub fn picks(&self) -> &[usize] {
        &self.answer.picked
    }

    /// Whether the text field is open.
    #[must_use]
    pub fn typing(&self) -> bool {
        self.answer.typing
    }

    /// What the text field holds.
    #[must_use]
    pub fn answer_text(&self) -> &str {
        &self.answer.draft.text
    }

    /// Why the last confirm answered nothing, when it answered nothing.
    #[must_use]
    pub fn answer_note(&self) -> Option<&str> {
        self.answer.note.as_deref()
    }

    /// Clears the answer, because no question is waiting any more or a different one is: a pick
    /// made for one question must never answer the next.
    pub fn clear_answer(&mut self) {
        self.answer = Answering::default();
    }

    /// Picks the option at `index`: replaces the pick for a question that takes one, toggles it
    /// for a question that takes several. False when no question offered that option.
    pub fn select_option(&mut self, index: usize) -> bool {
        let Waiting::Question { options, multiple } = self.waiting else {
            return false;
        };
        if index >= options || self.answer.typing {
            return false;
        }
        if multiple {
            match self
                .answer
                .picked
                .iter()
                .position(|picked| *picked == index)
            {
                Some(at) => {
                    self.answer.picked.remove(at);
                }
                None => self.answer.picked.push(index),
            }
        } else {
            self.answer.picked = vec![index];
        }
        self.answer.note = None;
        true
    }

    /// Puts `text` in the answer field, as typing would, and opens the field so Enter sends it.
    ///
    /// A headless caller — a test, an automation — has no key events to type with, so this is the
    /// seam it uses; the window's own path is the field's editing, which is Rune's business.
    pub fn set_answer_text(&mut self, text: impl Into<String>) {
        self.answer.typing = true;
        self.answer.draft = TextEdit::default();
        self.answer.draft.text = text.into();
        self.answer.note = None;
    }

    /// Opens or closes the text field a question can be answered in. False when nothing asks.
    pub fn toggle_typing(&mut self) -> bool {
        if !matches!(self.waiting, Waiting::Question { .. }) {
            return false;
        }
        self.answer.typing = !self.answer.typing;
        self.answer.note = None;
        true
    }

    /// Answers with the text field's contents, and reports whether an answer was sent. An empty
    /// field is refused in words and stays open, so nothing typed is lost.
    pub fn send_text(&mut self) -> bool {
        if !self.answer.typing {
            return false;
        }
        let text = self.answer.draft.text.trim().to_string();
        if text.is_empty() {
            self.answer.note =
                Some("nothing typed yet: type an answer, or Escape to go back".to_string());
            return false;
        }
        self.clear_answer();
        if let Some(live) = self.live.as_mut() {
            live.answer_text(text);
        }
        self.refresh();
        true
    }

    /// Returns from the text field to the options without answering.
    pub fn cancel_typing(&mut self) -> bool {
        if !self.answer.typing {
            return false;
        }
        self.answer.typing = false;
        self.answer.draft = TextEdit::default();
        self.answer.note = None;
        true
    }

    /// Answers the waiting question with every option picked. With none picked nothing is sent:
    /// the question stays up and the card says what is missing.
    pub fn confirm(&mut self) -> bool {
        if !matches!(self.waiting, Waiting::Question { .. }) {
            return false;
        }
        if self.answer.picked.is_empty() {
            let first = if self.waiting.multiple() {
                "toggle one with a number key"
            } else {
                "pick one with a number key"
            };
            self.answer.note = Some(format!("nothing picked yet: {first}, or press t to type"));
            return false;
        }
        let picks = std::mem::take(&mut self.answer.picked);
        self.answer.typing = false;
        self.answer.draft = TextEdit::default();
        self.answer.note = None;
        if let Some(live) = self.live.as_mut() {
            if let [index] = picks.as_slice() {
                live.pick(*index);
            } else {
                live.pick_many(&picks);
            }
        }
        self.refresh();
        true
    }

    /// Allows the waiting call for the rest of the session, so its tool does not ask again.
    pub fn allow_for_session(&mut self) {
        if let Some(live) = self.live.as_mut() {
            live.allow_for_session();
        }
        self.refresh();
    }
}
