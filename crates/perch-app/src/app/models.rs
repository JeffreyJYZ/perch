//! The provider's model list: what it offers, what window each model states, what it charges, and
//! the one thread that asks for it without the window waiting.
//!
//! The list is three things at once. The picker draws the ids, the meter divides by the context
//! window the provider states for the chosen one, and it turns the last turn's tokens into dollars
//! at the price stated for that same model — so the payload is read once and every answer is kept,
//! rather than asking the provider again for numbers it already sent.

use std::sync::Arc;

use perch_agent::{ListedModel, ModelTransport, Price};

use super::AppState;

/// What the provider stated about one model: its window and its price.
///
/// Both are optional and independent. A provider may state a window and no price, or a price and
/// no window, or neither; the meter shows what is known and never a number nobody sent.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ModelFacts {
    pub(crate) limit: Option<u32>,
    pub(crate) price: Option<Price>,
}

impl ModelFacts {
    /// The dollars a turn reporting `usage` cost, when this model's price is stated.
    fn cost(&self, usage: perch_agent::Usage) -> Option<f64> {
        Some(self.price?.cost(usage.input, usage.output))
    }
}

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
                    self.remember_models(models);
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
        self.facts()?.limit
    }

    /// What the active session's model charges, when the provider stated both rates.
    pub fn model_price(&self) -> Option<Price> {
        self.facts()?.price
    }

    /// The dollars the active session's last turn cost, when the provider stated the price.
    ///
    /// A session that has run no turn has no usage to price, so nothing is claimed; a provider
    /// that stated no price says nothing about money either.
    pub fn turn_cost(&self) -> Option<f64> {
        let session = self.sessions.get(self.active)?;
        self.facts()?.cost(session.usage()?)
    }

    /// What the provider stated for the active session's model.
    fn facts(&self) -> Option<&ModelFacts> {
        let model = &self.sessions.get(self.active)?.meta.model;
        self.limits.get(model)
    }

    /// Keeps every window and price the listing stated, so the meter does not ask again for
    /// numbers the provider already sent.
    fn remember_models(&mut self, models: &[ListedModel]) {
        for model in models {
            let price = Price::of(model.prompt_price, model.completion_price);
            if model.context.is_none() && price.is_none() {
                continue;
            }
            let facts = self.limits.entry(model.id.clone()).or_default();
            facts.limit = facts.limit.or(model.context);
            facts.price = facts.price.or(price);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// $0.15 per million in, $0.60 per million out.
    fn priced() -> ModelFacts {
        ModelFacts {
            limit: Some(200_000),
            price: Some(Price {
                prompt: 0.15 / 1_000_000.0,
                completion: 0.60 / 1_000_000.0,
            }),
        }
    }

    #[test]
    fn a_turn_is_priced_at_the_models_own_rates() {
        let cost = priced()
            .cost(perch_agent::Usage {
                input: 1_000,
                output: 500,
            })
            .expect("the rates are stated");

        assert!(
            (cost - 0.00045).abs() < 1e-12,
            "1000 in at $0.15/M and 500 out at $0.60/M is $0.00045, got {cost:?}"
        );
    }

    #[test]
    fn a_model_with_no_stated_price_costs_nothing_claimed() {
        assert_eq!(
            ModelFacts {
                limit: Some(200_000),
                price: None,
            }
            .cost(perch_agent::Usage {
                input: 9_000,
                output: 3_000,
            }),
            None,
            "a window with no price says nothing about money"
        );
    }
}
