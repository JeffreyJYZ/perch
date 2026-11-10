#!/bin/sh
# Builds the wasm plugin fixture checked into crates/perch-plugins/fixtures.
#
# The wasm is committed so the plugin tests need no wasm toolchain; run this only after editing
# the fixture plugin or the WIT world.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
crate="$root/crates/perch-plugins/fixtures/example-plugin"
out="$root/crates/perch-plugins/fixtures/example.wasm"
target_dir="${CARGO_TARGET_DIR:-$root/target-plugins}"

if ! rustup target list --installed | grep -qx wasm32-wasip2; then
	echo "installing the wasm32-wasip2 target" >&2
	rustup target add wasm32-wasip2
fi

CARGO_TARGET_DIR="$target_dir" cargo build \
	--manifest-path "$crate/Cargo.toml" \
	--target wasm32-wasip2 \
	--release

cp "$target_dir/wasm32-wasip2/release/example_plugin.wasm" "$out"
echo "wrote $out"
