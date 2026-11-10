//! Key encoding, resize plumbing and the flood budget.
//!
//! The paint test lives in `perch-ui`, because run-length grouping is the painter's job; these
//! prove the terminal side of the same task.

mod flood;
mod keys;
mod resize;
