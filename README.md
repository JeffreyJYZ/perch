# perch

A small native agent harness: Rune UI, Rust agent loop, Ghostty's terminal core.

## Build

- Rune is a local checkout: `perch-ui` depends on `../../experiments/rune/crates/rune` and `rune-kit`. Rune has no stable release, so the path is the pin.
- `libghostty-vt` vendors Ghostty and needs Zig 0.17.x on `PATH` (installed here).
- `scripts/gate.sh` is the gate: fmt, clippy with warnings denied, tests.

Spec: `docs/specs/2026-11-10-perch-design.md`. Plan: `docs/plans/2026-11-10-perch-core.md`.
