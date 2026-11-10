#!/bin/sh
set -eu

# The terminal crate vendors Ghostty, which only builds with the pinned Zig
# 0.15.2. Sourcing this first is what makes a clean shell build that crate.
_gate_dir=$(cd "$(dirname "$0")" && pwd)
if [ -f "$_gate_dir/zig-env.sh" ]; then
	. "$_gate_dir/zig-env.sh"
fi

# A source file has a budget: 200 lines, and a file already over it may only shrink.
"$_gate_dir/check-file-size.sh"
unset _gate_dir

# `cargo fmt --all` also formats local path dependencies, and Rune is not rustfmt-clean — so
# format exactly this workspace's members, read from cargo itself so a new crate cannot be
# forgotten the way perch-update was.
for _pkg in $(cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name'); do
	cargo fmt -p "$_pkg" -- --check
done
unset _pkg

cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
