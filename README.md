# perch

perch is a small native coding-agent harness: one Rust binary that opens a Rune window with a session list, a streaming transcript, file tabs with a read-only preview, and a Ghostty-backed terminal dock. The agent loop is OpenAI-compatible, its tools are confined to the session's worktree and gated by approvals, MCP servers are tool sources, and wasm plugins contribute tools and panels from a directory the app watches.

Spec: `docs/specs/2026-11-10-perch-design.md`. Plan and its status: `docs/plans/2026-11-10-perch-core.md`.

## Build

```sh
brew install zig@0.15   # keg-only on purpose; never `brew link` it
source scripts/zig-env.sh
cargo test
```

One shared `target/` for everything, including parallel work. Cargo already serialises on its lock, and a per-lane target directory costs 8–20 GiB each: fourteen of them reached 109 GiB of build output for this repo in a single day. Two scripts opt out on purpose, each into a directory of its own: `scripts/bundle.sh` uses `target-package/` and `scripts/live-probe.sh` uses `target-probe/`, so neither invalidates the dev tree.

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
- `--plugins <dir>` loads plugins from that directory instead of `<data>/plugins`.
- `--worktree <path>` names the directory this window is for: a session for it is opened when none exists for that path, and an existing one is left as it is. The window still opens on the most recent session.
- With no saved session, the first launch starts one on the launch directory when it is a project. A launch directory that is a filesystem root, `$HOME`, or an ancestor of `$HOME` is refused before it is read — a window opened from Finder starts at `/` — and the window opens with no session, a word saying why, and **Open folder**, which asks Rune's native folder picker. A session never creates or clones a worktree; it attaches to one that already exists.
- `--worktree` and a picked folder are a person's choice and are taken as given, a filesystem root and a home directory included. Only the guessed launch directory is refused.
- `dist/Perch.app` is the bundled build (see below); its executable is the same binary.

The centre's strip holds the sessions, then a tab per opened file, then `Settings`. A press on a file in the tree opens it in a tab and shows its read-only preview in the centre; Escape returns to the transcript with the file still selected. A press on `Settings` (or ⌘,) shows the settings form in the same place. The transcript and the terminal dock sit under that pane.

Data lives under `~/Library/Application Support/dev.jyz.perch` (the app id in `crates/perch-app/src/window/boot.rs`), which is where `settings.toml`, `secrets.json` and one `<id>.jsonl` per session live.

## Gate

```sh
./scripts/gate.sh
```

It sources `scripts/zig-env.sh` itself, then runs:

1. `scripts/check-file-size.sh` — a 200-line budget per Rust file under `crates/` (`src`, tests and examples). A file already over it is listed in `docs/refactor/oversized.txt` and may only shrink; a new offender fails the gate. The list is currently empty.
2. `cargo fmt -p <every workspace member> -- --check` — package by package, with the names read from `cargo metadata`, so a new crate cannot be forgotten; `cargo fmt --all` would also format the Rune path dependency, which is not rustfmt-clean.
3. `cargo clippy --workspace --all-targets -- -D warnings`.
4. `cargo test --workspace`.

Warnings fail. It needs `jq`, because the formatter's package list comes from `cargo metadata`. The tests need no network: provider streaming is replayed from recorded SSE fixtures, MCP servers are served by an in-repo fixture binary, and the terminal is driven through `replay` over recorded bytes.

## Bundle

```sh
scripts/bundle.sh
scripts/verify-bundle.sh
```

`scripts/bundle.sh` builds `perch-app` in release into `target-package/`, then stages `dist/Perch.app` by hand: the `Info.plist` version comes from the crate, the bundle id from `APP_ID` under `crates/perch-app/src`, the minimum macOS from the binary's own `minos` load command, and the icon is drawn by `scripts/make-icon.swift` and converted with `sips`/`iconutil`. It signs ad-hoc (`codesign --sign -`).

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
model = ""
theme = "dark"
# Empty means perch never checks for a release. A URL here is the manifest it reads.
update_check = ""

[[mcp_servers]]
name = "fixture"
command = "path/to/server"
args = []

[[mcp_servers]]
name = "remote"
url = "https://example.com/mcp"
```

The first-run defaults are written when the file is absent, and every field has a default so an older file still loads. The settings pane (the `Settings` tab, or ⌘,) edits the provider address, model, theme, update check URL and key; the four file-backed values are validated before anything is written, and a rejected field keeps the pane open and names itself. Picking a theme in that pane repaints the window before Save, and the saved word is what the window draws at the next launch. `mcp_servers` is hand-edited in the file; the pane does not show it.

The model field is a picker: **Refresh** asks the provider for its model ids and lists them, and an id can still be typed because a provider need not list its models. The composer's own model control opens the same pane in a session-only mode, so changing the model for the session does not rewrite the saved default. A save that includes a model makes it both the session's and the default.

The provider key is written to `secrets.json` in the data directory, `0600` and replaced through a temporary file, under the account `provider`. It is not part of `Settings`, so it has no path into `settings.toml`, and it never appears in a session file or a log line. The pane only ever says `stored` or `missing` — it never reads the value back — and an empty key on save keeps the one already stored, so changing a theme does not sign you out. A file rather than the keychain is deliberate: the keychain asks for approval per binary, and every ad-hoc-signed local build is a new binary, which answered "not stored" and sent turns out without an authorization header.

The environment is the fallback, for a window configured from the shell: `PERCH_API_KEY`, then `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY` for the key; `PERCH_BASE_URL`, then `OPENROUTER_BASE_URL`, `OPENAI_BASE_URL` when `settings.toml` names no address. A saved value always wins, and an empty saved address with no variable means the turn refuses rather than guessing an endpoint.

MCP OAuth tokens are the exception: they live in the keychain under service `dev.jyz.perch`, one JSON entry per server under `mcp:<server>`.

A remote `url` server is always authorized: a stored token is reused, and otherwise perch runs the browser flow (discovery, dynamic client registration, PKCE, loopback callback) before connecting. A remote server with no OAuth metadata fails and is reported, rather than being reached anonymously.

### Updates

The app checks for a release only when `update_check` names a manifest, and the field is in the settings pane too: empty is the default and means "do not check". The check runs on a thread of its own, started at launch and restarted by a save that changes the URL, so a slow server never holds a frame and a new address applies without a restart. The manifest's URL is the only thing that starts a request.

A release newer than this build becomes one line in the right-hand column with a Download button. A press fetches the archive, verifies its SHA-256 against the manifest, and writes it to `<data>/updates/perch-<version>.zip`. Installing is a manual step: quit perch, then unpack the archive over the bundle. The app never replaces the bundle it is running from.

### Reloading without a restart

The tools are rebuilt rather than restarted. `settings.toml` and the plugin directory are watched with a ~200 ms debounce (`crates/perch-app/src/watch.rs`); a change runs `Sources::rebuild`, which starts from an empty registry — the built-ins, then every `*.wasm` in the plugin directory, then every connected server — and swaps the result into the live registry every session dispatches through (`ArcSwap<ToolRegistry>`).

A call reads the registry when it starts, so a change lands on the next call: a turn already in flight finishes its current call against the tools it began with and sees the new list for its next one. No message is rewritten and no turn is rewound. Deleting a plugin or a server removes its tools from `specs()` and from dispatch at that point; nothing is left to resurrect it. A plugin that will not load is one line in the right-hand column, never a failure to open the window. MCP connections outlive a rebuild: `McpHost::reconcile` keeps a server whose name and configuration are unchanged, connects a new or changed one, and drops one that is no longer configured or has died — including a server that never connected at startup, which the host records. `Sources::settle` runs `McpHost::disconnect` whenever the worker publishes a snapshot, so a crashed server's tools leave the registry at the next frame.

The model's tool list is read from the live registry when each request starts, so a plugin installed mid-session is offered to the model on its next request, with no new session and no restart. A request already out carries the list it was sent with.

## What is not there yet

- The bundle is `arm64` and ad-hoc signed: no notarization and no universal build, so Gatekeeper rejects it on any Mac but the one that built it (see Bundle).
- The update check stages and verifies a download; installing is the person's step after perch quits. Nothing swaps the running bundle.
- A session's `branch` is stored empty: the app does not read the worktree's git branch, so the row says "no branch". Ahead/behind and a diff view do not exist.
- The file tab's close control acts on the press; the release then lands on the tab that reflows under the pointer, so closing the last file tab ends on `Settings`. `Shell::close_tab` itself returns to the session; the press geometry is rune-kit's.
- The accessibility tree names regions, text rows, and the terminal's caret and selection; it does not name individual terminal cells.
- The MCP host records every call, but the window shows a call only as its namespaced tool card; the host's connection and call notes are not drawn anywhere, and only a failed server is said in the right-hand column.
