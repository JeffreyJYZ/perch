//! Saved permission rules, split by what each case proves: they survive a restart, they never
//! widen, and a corrupt file is visible instead of silently allowing everything.

mod corrupt;
mod harness;
mod saved;
mod widening;
