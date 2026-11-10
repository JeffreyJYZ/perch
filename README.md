# perch

perch is a small native coding-agent harness: one Rust binary that opens a Rune window with a session list, a streaming transcript, a worktree tree and a Ghostty-backed terminal dock. The agent loop is OpenAI-compatible, its tools are confined to the session's worktree and gated by approvals, MCP servers are tool sources, and wasm plugins contribute tools and panels from a directory the app watches.

Spec: `docs/specs/2026-11-10-perch-design.md`. Plan and its status: `docs/plans/2026-11-10-perch-core.md`.

## Build

```sh
brew install zig@0.15   # keg-only on purpose; never `brew link` it
source scripts/zig-env.sh
cargo test
```

One shared `target/` for everything, including parallel work. Cargo already serialises on
its lock, and a per-lane target directory costs 8–20 GiB each: fourteen of them reached
109 GiB of build output for this repo in a single day.

`source scripts/zig-env.sh` is required in every fresh shell before anything that builds `perch-term`. It is sourced, never executed, and it is idempotent.

Why Zig 0.15.2 and not the 0.17 on `PATH`:

- `perch-term` pins `libghostty-vt = "=0.2.2"`, which vendors Ghostty at `a887df42`. That tree's `build.zig.zon` sets `minimum_zig_version = "0.15.2"`, and Zig 0.17.1 is refused at configure time, so a newer Zig is not a fallback — it is a build failure.
- `scripts/zig-env.sh` puts the keg-only 0.15.2 first on `PATH` (via `brew --prefix zig@0.15`, or `$PERCH_ZIG`), and exports `ZIG`.
- macOS 27's SDK lists only `arm64e` in its libSystem stubs, which a Zig 0.15.2 link can fail on. The script probes one actual link and, when it fails, caches a corrected SDK shim plus an `xcrun` in the gitignored `.zig-env/`. The probe result is cached beside it.
- The vendored parser is always built `ReleaseFast` (`LIBGHOSTTY_VT_SYS_OPTIMIZE`); the Rust side follows the cargo profile.

`rune` and `rune-kit` are path dependencies on `../../experiments/rune/crates/*`, because Rune has no stable release. The path is the pin.

## Run

```sh
cargo run -p perch-app --bin perch
```

- `--tree` prints the window's accessibility tree as JSON and exits; it reads no settings and starts no shell.
- `--offline` opens the window with the session list and the dock only, skipping the app.
- With no saved session, the first launch creates one bound to the current directory. A session never creates or clones a worktree; it attaches to one that already exists.
- `dist/Perch.app` is the bundled build (see below); its executable is the same binary.

Data lives under `~/Library/Application Support/dev.jyz.perch` (the app id from `crates/perch-app/src/main.rs`), which is where `settings.toml` and one `<id>.jsonl` per session live.

## Gate

```sh
./scripts/gate.sh
```

It sources `scripts/zig-env.sh` itself, then runs:

1. `scripts/check-file-size.sh` — a 200-line budget per source file. A file already over it is listed in `docs/refactor/oversized.txt` and may only shrink; a new offender fails the gate.
2. `cargo fmt -p <each of the nine packages> -- --check` — package by package, because `cargo fmt --all` would also format the Rune path dependency, which is not rustfmt-clean.
3. `cargo clippy --workspace --all-targets -- -D warnings`.
4. `cargo test --workspace`.

Warnings fail. The tests need no network: provider streaming is replayed from recorded SSE fixtures, and the terminal is driven through `replay` over recorded bytes.

## Bundle

```sh
scripts/bundle.sh
scripts/verify-bundle.sh
```

`scripts/bundle.sh` builds `perch-app` in release into its own `target-package/` directory, then stages `dist/Perch.app` by hand: the `Info.plist` version comes from the crate, the bundle id from `APP_ID` in `main.rs`, the minimum macOS from the binary's own `minos` load command, and the icon is drawn by `scripts/make-icon.swift` and converted with `sips`/`iconutil`. It signs ad-hoc (`codesign --sign -`).

`scripts/verify-bundle.sh` prints one verdict per check: bundle present, plist valid, identifier/version/name/executable match their sources of truth, `codesign --verify --deep --strict` passes, the binary is a Mach-O executable, its architecture is `arm64`, and the icon decodes at 1024 px.

Two limits are real:

- The signature is ad-hoc, so **Gatekeeper rejects the bundle on any other Mac** until it is signed with a Developer ID identity and notarized. This repository does not do either.
- The staged bundle is `arm64` only; the verifier requires that. There is no Intel build.

## Live probe

```sh
scripts/live-probe.sh
```

This runs the real turn loop against OpenRouter's free models, one at a time: `cohere/north-mini-code:free`, then `nvidia/nemotron-3-ultra-550b-a55b:free`, then `openrouter/free`. A model that fails on tool calling is a printed status and the next attempt; the script exits 0 on the first model that completes a multi-round turn that writes and reads a file in a temporary directory.

- Needs `jq`.
- The key comes from `$OPENROUTER_API_KEY`, else `.openrouter.key` in `~/.local/share/opencode/auth.json` (respecting `XDG_DATA_HOME`).
- The key is **never printed** and is handed to the probe through the environment, never on the command line, so it stays out of `ps`. Every line the probe prints is a name, a count or a verdict.
- The probe example is built into `target-probe/`, separate from the dev tree.

## Settings and secrets

`settings.toml` in the data directory holds only non-secret values:

```toml
provider_base_url = "https://openrouter.ai/api/v1"
model = "deepseek/deepseek-v4.1-flash"
theme = "dark"

[[mcp_servers]]
name = "fixture"
command = "path/to/server"
args = []

[[mcp_servers]]
name = "remote"
url = "https://example.com/mcp"
```

The first-run defaults are written when the file is absent, and every field has a default so an older file still loads. The settings panel (⌘, or the gear) edits the provider address, model, theme and key, and validates the first three before writing anything; a rejected field keeps the panel open and names itself. `mcp_servers` is hand-edited in the file, not in the panel.

The provider key is written to the macOS keychain under service `dev.jyz.perch`, account `provider`. It is never a field of `Settings`, never appears in `settings.toml`, a session file or a log line, and the panel only ever says `stored` or `missing` — it never reads the value back. An empty key on save keeps the one already stored, so changing a theme does not sign you out.

MCP OAuth tokens live in the same keychain, one entry per server under `mcp:<server>`.

### Reloading without a restart

The tools are rebuilt rather than restarted. `settings.toml`, the plugin directory and the `mcp_servers` list are watched with a ~200 ms debounce (`crates/perch-app/src/watch.rs`); a change runs `Sources::rebuild`, which starts from an empty registry — the built-ins, then every `*.wasm` in the plugin directory, then every connected server — and swaps the result into the live registry every session dispatches through (`ArcSwap<ToolRegistry>`).

A call reads the registry when it starts, so a change lands on the next call: a turn already in flight finishes its current call against the tools it began with and sees the new list for its next one. No message is rewritten and no turn is rewound. Deleting a plugin or a server removes its tools from `specs()` and from dispatch at that point; nothing is left to resurrect it. A plugin that will not load is one line in the right-hand column, never a failure to open the window. MCP connections outlive a rebuild: `McpHost::reconcile` keeps a server whose name and configuration are unchanged, connects a new or changed one, and drops one that is no longer configured or has died — including a server that never connected at startup, which the host now records. A crashed server's tools leave the registry at the next frame.

The model's tool list is still fixed when a session starts (`AgentConfig.tools` is passed to `perch-agent` at spawn). A plugin installed mid-session answers immediately when called and appears in `specs()`, but the model is offered it from the next session, not the next turn.

A remote `url` server is always authorized: a stored token is reused, and otherwise perch runs the browser flow (discovery, dynamic client registration, PKCE, loopback callback) before connecting. A remote server with no OAuth metadata fails and is reported, rather than being reached anonymously.

## What is not there yet

- No updater wired into the app: `perch-update` checks a manifest, orders prerelease versions, verifies a SHA-256 download and stages it, but nothing in the window calls it yet. No notarized or universal bundle — the build is arm64 and ad-hoc signed.
- The plugin host loads from `<data>/plugins` (or `--plugins <dir>`), and its tools and panels are live: a file added, edited or deleted is picked up by the directory watcher.
- The worktree tree is display-only: clicking a file does nothing, the preview panel is not placed in the window, and the "Open in Zed" button has no press. The crate underneath (`perch-files`) does have the walk, status, preview and `open -a Zed`.
- A question can be answered in the window only by picking one numbered option; multi-pick and free text exist in the agent layer, not in the window. "Allow for session" is likewise not offered by the window (Enter allows once, Escape refuses).
- Tool cards stay collapsed: the transcript can expand one, but no press is wired to a card.
- No wheel handler is attached to the transcript, so a long transcript cannot be scrolled with the wheel; only the terminal grid takes a wheel. The accessibility tree names regions and text rows rather than individual cells or the cursor.
- A session's branch is stored empty, and the model's tool list is fixed when a session starts, so a plugin installed mid-session answers on the next call but is offered to the model only from a new session.
