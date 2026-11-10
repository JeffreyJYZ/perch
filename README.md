# perch

A small native agent harness: Rune UI, Rust agent loop, Ghostty's terminal core.

## Build

Run `source scripts/zig-env.sh` once per shell, then `cargo test` and `cargo run` work; the toolchain comes from `brew install zig@0.15` and the script uses its keg path, never `brew link`.

- Zig 0.15.2 is exact, not a range: `libghostty-vt-sys` 0.2.2 vendors Ghostty at `a887df42`, whose `build.zig.zon` sets `minimum_zig_version = "0.15.2"`, and the linked 0.17.1 is refused at configure time.
- macOS 27's SDK lists only `arm64e` in its libSystem stubs, which a Zig 0.15.2 build can fail to link, so the script probes one link and writes a corrected SDK shim into the gitignored `.zig-env/` when it is needed.
- Rune is a local checkout: `perch-ui` depends on `../../experiments/rune/crates/rune` and `rune-kit`. Rune has no stable release, so the path is the pin.
- `scripts/gate.sh` is the gate: fmt, clippy with warnings denied, tests. It sources `scripts/zig-env.sh` itself, so the workspace test builds the terminal crate too.

Spec: `docs/specs/2026-11-10-perch-design.md`. Plan: `docs/plans/2026-11-10-perch-core.md`.
