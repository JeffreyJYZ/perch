//! The model a session asks for, readable and changeable while the session runs.
//!
//! A [`Model`] is a handle, not a value: every clone shares one id, so the session that changes it
//! changes the very next request and never one already streaming. That is what makes picking a
//! model in the composer a per-session choice with no agent restarted and no history lost.

use std::sync::{Arc, RwLock};

/// The model id one agent asks for.
#[derive(Clone, Debug, Default)]
pub struct Model {
    id: Arc<RwLock<String>>,
}

impl Model {
    /// A handle holding `id`.
    pub fn new(id: impl Into<String>) -> Model {
        Model {
            id: Arc::new(RwLock::new(id.into())),
        }
    }

    /// The id a request started now would ask for.
    pub fn current(&self) -> String {
        self.read().clone()
    }

    /// Replaces the id. A request already out keeps the model it was sent with.
    pub fn set(&self, id: impl Into<String>) {
        let mut held = match self.id.write() {
            Ok(held) => held,
            // A poisoned lock still holds the id; no code here panics while holding it.
            Err(poisoned) => poisoned.into_inner(),
        };
        *held = id.into();
    }

    /// Whether no model is chosen, which is what refuses a turn in words.
    pub fn is_empty(&self) -> bool {
        self.read().trim().is_empty()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, String> {
        match self.id.read() {
            Ok(held) => held,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl From<&str> for Model {
    fn from(id: &str) -> Model {
        Model::new(id)
    }
}

impl From<String> for Model {
    fn from(id: String) -> Model {
        Model::new(id)
    }
}
