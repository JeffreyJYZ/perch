#!/bin/sh
set -eu

# The terminal crate vendors Ghostty, which only builds with the pinned Zig
# 0.15.2. Sourcing this first is what makes a clean shell build that crate.
_gate_dir=$(cd "$(dirname "$0")" && pwd)
if [ -f "$_gate_dir/zig-env.sh" ]; then
	. "$_gate_dir/zig-env.sh"
fi
unset _gate_dir

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
