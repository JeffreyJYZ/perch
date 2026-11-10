#!/bin/sh
# Builds the wasm plugin fixtures checked into crates/perch-plugins/fixtures.
#
# The wasm is committed so the plugin tests need no wasm toolchain; run this only after editing a
# fixture plugin or the WIT world. `counter.wasm` is the only plugin the happy-path tests load;
# the colliding and trapping fixtures live in subdirectories their own tests point at, which the
# host's `*.wasm` scan of the fixtures root never reaches.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
fixtures="$root/crates/perch-plugins/fixtures"
target_dir="${CARGO_TARGET_DIR:-$root/target-plugins}"

if ! rustup target list --installed | grep -qx wasm32-wasip2; then
	echo "installing the wasm32-wasip2 target" >&2
	rustup target add wasm32-wasip2
fi

# build <fixture crate dir> <crate's wasm stem> <path under fixtures/>
build() {
	CARGO_TARGET_DIR="$target_dir" cargo build \
		--manifest-path "$fixtures/$1/Cargo.toml" \
		--target wasm32-wasip2 \
		--release
	mkdir -p "$(dirname "$fixtures/$3")"
	cp "$target_dir/wasm32-wasip2/release/$2.wasm" "$fixtures/$3"
	echo "wrote $fixtures/$3"
}

build example-plugin example_plugin counter.wasm
build shadow-plugin shadow_plugin shadow/shadow.wasm
build dupe-plugin-a dupe_plugin_a duplicate/01-first.wasm
build dupe-plugin-b dupe_plugin_b duplicate/02-second.wasm
build trap-panel-plugin trap_panel_plugin trap-panel/trap-panel.wasm
