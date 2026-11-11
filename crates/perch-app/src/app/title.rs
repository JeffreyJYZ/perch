//! Naming a session after its first turn: when to ask, who has been asked, and what lands.
//!
//! The job is started once per session, and only while the title is still the folder name it was
//! created with — so a `/rename` always wins and a session is never named twice. The request and
//! the cleaning of the answer are [`title_ask`](super::title_ask) and
//! [`title_words`](super::title_words); this file is the bookkeeping and the applying.

use std::sync::mpsc::{Receiver, TryRecvError};

use perch_agent::ChatRequest;
use perch_store::SessionEvent;

use super::AppState;
use super::title_ask::{ask, request};
use super::title_words::{default_title, tidy};
use crate::session::SessionRuntime;

/// The title requests this app has made: who was asked, and what is still on its way.
#[derive(Default)]
pub(crate) struct Titles {
    /// The sessions a title was asked for, so a second turn never asks again.
    asked: std::collections::HashSet<String>,
    jobs: std::collections::HashMap<String, Receiver<Result<String, String>>>,
}

impl AppState {
    /// Starts a title request for every session whose first turn just ended, once. Answers
    /// whether one was started, which is what keeps frames coming while it is in flight.
    pub(crate) fn start_titles(&mut self) -> bool {
        let mut started = Vec::new();
        for runtime in &self.sessions {
            let id = runtime.meta.id.clone();
            if self.titles.asked.contains(&id) || self.titles.jobs.contains_key(&id) {
                continue;
            }
            if runtime.turns_ended() != 1 || runtime.meta.title != default_title(&runtime.meta) {
                continue;
            }
            if let Some(request) = request(runtime, &self.defaults) {
                started.push((id, request));
            }
        }
        let mut any = false;
        for (id, request) in started {
            self.titles.start(id, request);
            any = true;
        }
        any
    }

    /// Takes whatever title answers have arrived. A success renames the session and rewrites the
    /// header; a failure writes one log line and leaves the folder name in place.
    pub(crate) fn absorb_titles(&mut self) -> bool {
        let mut arrived = Vec::new();
        for runtime in &mut self.sessions {
            let id = runtime.meta.id.clone();
            let Some(job) = self.titles.jobs.get(&id) else {
                continue;
            };
            match job.try_recv() {
                Ok(answer) => arrived.push((id, answer)),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    arrived.push((id, Err("the title request stopped".to_string())));
                }
            }
        }
        let mut changed = false;
        for (id, answer) in arrived {
            self.titles.jobs.remove(&id);
            let Some(runtime) = self.sessions.iter_mut().find(|one| one.meta.id == id) else {
                continue;
            };
            match answer.and_then(|raw| tidy(&raw).ok_or_else(|| "it named nothing".to_string())) {
                Ok(title) => {
                    runtime.meta.title = title.clone();
                    if let Err(error) = runtime.log.set_title(&title) {
                        log_line(runtime, &format!("title: not saved: {error}"));
                    }
                    changed = true;
                }
                Err(words) => log_line(runtime, &format!("title: {words}")),
            }
        }
        changed
    }

    /// Forgets a deleted session's title state, so a reused id is not treated as asked.
    pub(crate) fn titles_forget(&mut self, id: &str) {
        self.titles.asked.remove(id);
        self.titles.jobs.remove(id);
    }

    /// How many sessions still have a title request in flight. A test reads it; the window uses
    /// [`AppState::busy`] so frames keep coming until the answer lands.
    pub fn titles_pending(&self) -> usize {
        self.titles.jobs.len()
    }
}

impl Titles {
    /// Marks the session asked and puts the request on a thread of its own, so no frame waits.
    fn start(&mut self, id: String, request: ChatRequest) {
        self.asked.insert(id.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("perch-title".to_string())
            .spawn(move || {
                let _ = tx.send(ask(&request));
            })
            .expect("spawn the title request");
        self.jobs.insert(id, rx);
    }
}

/// One line in the session's log, never in the transcript.
fn log_line(runtime: &mut SessionRuntime, words: &str) {
    let _ = runtime.log.append(&SessionEvent::Note {
        text: words.to_string(),
    });
}
