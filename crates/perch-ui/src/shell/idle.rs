//! The window with no app behind it: the layout, the file panel and the dock alone.
//!
//! A `--tree` dump and a layout test run against this, so neither needs a data directory, a
//! keychain or a provider. Every press is answered in words the same way the real window would
//! answer one it could not take.

use std::time::Duration;

use crate::transcript::Transcript;

use super::live::{Live, Waiting};

/// A window with no app behind it.
pub struct Idle;

impl Live for Idle {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Err("no session is open".to_string())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
}
