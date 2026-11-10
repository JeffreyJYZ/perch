//! The answer card, laid out and driven: what it says its keys do, and what a press does to it.
//!
//! The app behind the window is a fake that reports whatever wait a test asks for, so the presses
//! still go through the window's own map and the answer field is still Rune's text box: what is
//! exercised here is the wiring, not a helper.

mod card;
mod typing;

use std::time::Duration;

use perch_ui::shell::{Live, Panels, Waiting, window_key};
use perch_ui::{Shell, Transcript};
use rune::{Env, Key, KeyEvent, Mods, Runtime};
use serde_json::Value;

use crate::harness::{WINDOW, meta};

/// A question that takes one option.
pub const QUESTION: Waiting = Waiting::Question {
    options: 2,
    multiple: false,
};
/// The same question taking several.
pub const MULTI: Waiting = Waiting::Question {
    options: 3,
    multiple: true,
};

/// An app that waits on what the test says, and remembers a typed answer.
#[derive(Default)]
pub struct Asking {
    pub waiting: Waiting,
    pub answered: Option<String>,
}

impl Panels for Asking {}

impl Live for Asking {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        self.waiting
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Ok(())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
    fn answer_text(&mut self, text: String) {
        self.answered = Some(text);
    }
}

/// The window over a fake that is waiting on `waiting`, laid out once.
pub fn laid_out(waiting: Waiting) -> Runtime<Shell<Asking>> {
    let shell = Shell::new(vec![meta("work")], 0).with_live(Asking {
        waiting,
        answered: None,
    });
    let mut runtime = Runtime::new(shell, perch_ui::shell::view, WINDOW, Env::new(true, 2.0));
    runtime.on_key = Some(|shell: &mut Shell<Asking>, ev, tx| window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

/// One typed character, as the window's map sees it.
pub fn typed(runtime: &mut Runtime<Shell<Asking>>, value: &str) {
    runtime.key(KeyEvent {
        key: Key::Text(value.to_string()),
        mods: Mods::default(),
    });
    runtime.frame(0.0);
}

/// One press with no modifiers.
pub fn press(runtime: &mut Runtime<Shell<Asking>>, key: Key) {
    runtime.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
}

/// Everything the tree says, so a test can assert on a word the card draws.
pub fn tree(runtime: &Runtime<Shell<Asking>>) -> String {
    runtime.accessibility_json().to_string()
}

/// The node keyed `key`, wherever it sits in the tree.
pub fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, key))
}
