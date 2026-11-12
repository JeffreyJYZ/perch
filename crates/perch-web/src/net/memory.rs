//! A fetcher that answers from a fixture table, so every test is offline and deterministic.

use std::sync::Mutex;

use super::{FetchError, Fetcher, Response};

/// Answers the first request whose URL contains a recorded needle, and remembers what was asked.
///
/// The needle is a substring rather than a full URL so a test states only what it depends on
/// (a host, a path, a query) and not the exact encoding the tool happens to produce.
#[derive(Default)]
pub(crate) struct MemoryFetcher {
    answers: Mutex<Vec<(String, Result<Response, FetchError>)>>,
    asked: Mutex<Vec<String>>,
}

impl MemoryFetcher {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn answer(self, needle: &str, response: Response) -> Self {
        self.record(needle, Ok(response))
    }

    pub(crate) fn fail(self, needle: &str, error: FetchError) -> Self {
        self.record(needle, Err(error))
    }

    /// Every URL this fetcher was asked for, in order.
    pub(crate) fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("asked lock").clone()
    }

    fn record(self, needle: &str, answer: Result<Response, FetchError>) -> Self {
        self.answers
            .lock()
            .expect("answers lock")
            .push((needle.to_string(), answer));
        self
    }
}

impl Fetcher for MemoryFetcher {
    fn get(&self, url: &str) -> Result<Response, FetchError> {
        self.asked.lock().expect("asked lock").push(url.to_string());
        let answers = self.answers.lock().expect("answers lock");
        answers
            .iter()
            .find(|(needle, _)| url.contains(needle.as_str()))
            .map(|(_, answer)| answer.clone())
            .unwrap_or_else(|| {
                Err(FetchError::Transport {
                    url: url.to_string(),
                    reason: "no answer recorded for this URL".into(),
                })
            })
    }
}
