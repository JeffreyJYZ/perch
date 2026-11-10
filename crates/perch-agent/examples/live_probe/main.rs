//! One live turn against a real provider, for a model named on the command line.
//!
//! The unit tests replay recorded SSE bodies; this runs the same [`Agent`] against
//! OpenRouter, with the six built-ins confined to a fresh temporary directory and an
//! interactor that allows everything. The prompt forces at least two rounds: a write, a
//! read, an append, and a sentence that can only be written after the tool results came
//! back. Anything printed is a name, a count or a verdict — never the key, never a
//! request header.
//!
//! The key arrives in `OPENROUTER_API_KEY`, never on the command line, so it cannot be
//! read back out of the process table. Exit code 0 means the loop genuinely ran.

mod drive;
mod probe;

fn main() {
    std::process::exit(probe::run());
}
