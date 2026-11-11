//! The provider's model list: what it offers, what window each model states, and the one thread
//! that asks for it without the window waiting.
//!
//! The list is two things at once. The picker draws the ids, and the meter divides by the context
//! window the provider states for the chosen one — so the payload is read once and both answers
//! are kept, rather than asking the provider again for a number it already sent.

use std::sync::Arc;

use perch_agent::{ListedModel, ModelTransport};

use super::AppState;

impl AppState {
    /// The models the provider lists, sorted, through whatever transport this app was opened with.
    ///
    /// The error is why nothing was listed, in words — a provider without the endpoint answers
    /// 404 here, and the panel says so and falls back to typing an id.
    pub fn list_models(&self) -> std::result::Result<Vec<ListedModel>, String> {
        perch_agent::list_models_with(
            &*self.listings,
            &self.defaults.base_url,
            &self.defaults.api_key,
        )
        .map_err(|error| error.to_string())
    }

    /// Replaces the model list's transport, as a test does so no socket is opened.
    #[must_use]
    pub fn with_listings(mut self, transport: Arc<dyn ModelTransport>) -> Self {
        self.listings = transport;
        self
    }

    /// Starts asking the provider what it offers, on its own thread, and returns at once.
    ///
    /// The panel opens on this click; waiting for the provider here is what made opening it take
    /// a second, and a menu that appears a second after the press reads as a broken window.
    pub fn start_models(&mut self) {
        if self.models_job.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let listings = Arc::clone(&self.listings);
        let base_url = self.defaults.base_url.clone();
        let api_key = self.defaults.api_key.clone();
        std::thread::spawn(move || {
            let listed = perch_agent::list_models_with(&*listings, &base_url, &api_key)
                .map_err(|error| error.to_string());
            // A closed channel means the window is gone: the answer has nobody to go to.
            let _ = tx.send(listed);
        });
        self.models_job = Some(rx);
    }

    /// The answer to [`AppState::start_models`], when it has arrived.
    pub fn models_ready(&mut self) -> Option<std::result::Result<Vec<ListedModel>, String>> {
        let job = self.models_job.as_ref()?;
        match job.try_recv() {
            Ok(listed) => {
                self.models_job = None;
                if let Ok(models) = &listed {
                    self.remember_limits(models);
                }
                Some(listed)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.models_job = None;
                Some(Err("the model list stopped without an answer".to_string()))
            }
        }
    }

    /// The context window the provider states for the active session's model, when it states one.
    ///
    /// The number comes from the last listing read, so a window that has never asked the provider
    /// has no limit yet and the meter says only how many tokens went out.
    pub fn model_limit(&self) -> Option<u32> {
        let model = &self.sessions.get(self.active)?.meta.model;
        self.limits.get(model).copied()
    }

    /// Keeps every limit the listing stated, so the meter does not ask again for a number the
    /// provider already sent.
    fn remember_limits(&mut self, models: &[ListedModel]) {
        for model in models {
            if let Some(limit) = model.context {
                self.limits.insert(model.id.clone(), limit);
            }
        }
    }
}
