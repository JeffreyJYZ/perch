# perch — design

Date: 2026-11-10
Status: shipped as 0.1.0 — the core harness exists and is gated; the gaps are listed under "Shipped and not shipped". This document describes the code that is in the tree, not the plan that preceded it.

## Intent

opencode is the best agent UI I use and the heaviest thing I run. perch is the harness I keep open instead: one native window, no Node runtime, no browser engine, that keeps the two things I leave opencode for — seeing the worktree and browsing its files, and a terminal in the same window — and treats plugins as part of the product from the first version.

Everything is Rust, and the UI is Rune.

## Goals

- A single Rust binary that opens a window and starts a session in under a second.
- A transcript that streams assistant text and tool calls, with one card per call and selection that spans the column.
- A real terminal: full-screen programs, colours, scrollback, correct key encoding, resize, bracketed paste and selection copy.
- A file browser that reflects the worktree and git state, previews a file read-only, and hands it to Zed on request.
- A model that can ask the user a question with options and a free-text answer, and pause the turn until it is answered.
- MCP servers as tool sources: local stdio, remote HTTP, and the OAuth flow, with MCP tools allowed to run without an approval prompt but every call recorded.
- Plugins from day one: a plugin can add tools the model calls and declare a side panel.
- A session is bound to a worktree that already exists on disk.
- One `settings.toml` for values, the macOS keychain for secrets, and one append-only JSONL log per session.

## Non-goals

- Editing files in the app. Files are read-only; edits belong to the agent or to Zed.
- Creating or deleting worktrees, branches or repositories.
- LSP, debuggers, run configurations, build panes.
- Provider-specific APIs beyond OpenAI-compatible streaming.
- Windows and Linux.
- Notarization, DMG layout and auto-update in this version.

## Constraints as they turned out

- Rune ships `dev` only, so `perch-ui` and `perch-app` depend on the local checkout at `../../experiments/rune/crates/{rune,rune-kit}`. There is no registry pin to float.
- The terminal core is `libghostty-vt` 0.2.2 (MIT) through its Rust wrapper, pinned exactly. Its vendored Ghostty is built from source, so **Zig is required**: the vendored tree sets `minimum_zig_version = "0.15.2"` and refuses 0.17.1 at configure time. `scripts/zig-env.sh` sources the keg-only 0.15.2 and, on macOS 27 where the SDK's libSystem stubs list only `arm64e`, writes a cached SDK shim into `.zig-env/`.
- No Node, no webview, no JavaScript.
- macOS only. The bundle script stages an `arm64` binary; there is no Intel build or universal artefact, so the original "Apple silicon and Intel" reading is not true of what ships.

## Approaches considered

**One process, threads, channels — chosen.** The agent loop, provider stream, tool execution, dispatch thread and PTY readers run on ordinary threads and hand events to the UI through channels. Rune's live values carry the latest state into each frame. A misbehaving component takes the window with it.

**UI process plus a background daemon.** Crash isolation, and a headless mode later. Rejected: it requires a socket protocol with versioning and reconnection before there is anything worth isolating.

**Agent as a supervised child process.** Cheaper than a socket, but cancellation, streaming and session ownership get awkward exactly where correctness matters most.

**Terminal, three ways.** Draw our own VT grid (rejected: months of edge cases), embed a web terminal (rejected: that is the technology the project is leaving), or use Ghostty's extracted core (`libghostty-vt`) and draw its cells ourselves (chosen). It provides parsing, grid, scrollback, styles, formatting and key/mouse encoding, and leaves drawing to the host.

**Plugins, four ways.** Luau (adds a second language to an all-Rust project), native Rust dylibs (a bad plugin can corrupt the app, and every plugin needs a rebuild per OS), separate processes (language-agnostic and isolated, but heavier per call and no UI access without inventing a protocol), Wasm components via wasmtime (chosen: sandboxed, written in Rust, one ABI, hot-loadable).

**Sessions and worktrees.** Creating worktrees makes the app own cleanup, locking and uncommitted work; no worktree concept lets two sessions overwrite each other; a session binds to an existing worktree (chosen).

## Architecture

One binary, one window, four teams of threads, plus two hosts.

- **UI (Rune, main thread).** Draws the frame, owns focus, keybindings and panels. Reads the latest state through live values; never blocks on IO. The window is 1280×820 points, a titled window rather than full-size content.
- **Agent.** Owns the turn: message list, provider request, streamed deltas, tool-call assembly, rounds, cancellation, token counts. Emits typed events; never touches UI state directly.
- **Dispatch.** One thread per session runs every tool call, asks the user for approvals and answers, and sends the result back to the turn. A turn that is waiting on a person blocks only that thread.
- **Terminal.** One thread per terminal tab owns both the PTY reader and the Ghostty grid, because `libghostty-vt` is neither `Send` nor `Sync`; it publishes plain `Frame` snapshots that the UI paints.
- **MCP host.** One Tokio runtime, because `rmcp` is async and the tool trait is not. Calls enter it with `block_on` from the caller's thread.
- **Plugin host.** wasmtime component instances with bounded fuel and wall time; a trap disables that plugin and removes its tools.

Events are typed enums on a channel. The UI keeps the last known value per panel; nothing re-derives state from scratch per frame.

Persistence: one JSONL file per session, `<data dir>/<id>.jsonl`, whose first line is the session header (id, title, worktree, branch, model, created) and whose later lines are user and assistant text, tool calls, tool results and turn endings. Appends write whole lines, so a crash costs at most the line being written, and a torn line is dropped on read. Settings live in one TOML file; provider keys and MCP OAuth tokens live in the macOS keychain.

Sessions: the left column lists sessions and a tab strip switches them. The active tab owns its transcript, its terminal tabs and its file tree root. Switching is a state swap, never a reload from disk; the list is read from the JSONL headers at start so the window opens with history. A session whose worktree has gone refuses new turns and says why in its own transcript. `branch` is written empty today: the app does not read the worktree's git branch, so the session row says "no branch".

## Components

Layout: one column to the left, full window height, holding the session list above the file tree; the transcript fills the centre with the prompt under it; the terminal dock sits under the transcript only, so it belongs to the session it shows. State is written in words — "waiting", "done", "failed", "approval needed", "missing", "stored" — never a colour alone.

### Agent loop

Turn state machine: idle → requesting → streaming → tool calls → waiting for approval or an answer → next request → done, plus cancelled and errored. The provider client is one OpenAI-compatible streaming implementation over SSE, reading `choices[].delta`, tool-call deltas and usage, with a 120 s budget per request. Base URL, key and model come from settings, so OpenRouter, Command Code and local servers all work with no extra code.

A turn is a short conversation, not one request: every call is answered and the next request carries the assistant's call and the tool messages beside it. It ends when the model answers without calling anything, at `MAX_ROUNDS` (12), on cancellation or on an error. Tool-call fragments are assembled per index and parsed once at the end of the stream; a parse failure ends the turn errored rather than yielding half an argument set. Cancellation is cooperative and always produces exactly one `TurnEnd`.

### Tools

`read`, `write`, `edit`, `bash`, `list`, `grep` — six built-ins, registered rather than enumerated: `ToolRegistry::call` dispatches by name only, and `specs()` is computed from what is registered, so a plugin or MCP tool takes the same path.

Contract: `bash` runs with the worktree as cwd under a 30 s timeout with both streams captured and output truncated at 16 KiB; `read` caps at 64 KiB and numbers lines; `grep` launches `rg` under the same timeout and reports a missing `rg` rather than an empty result; `list` caps at 512 entries; `edit` returns a unified diff; `preview` is computed without running the tool, and a preview that fails denies the call.

Permissions: `write`, `edit` and `bash` require approval with a preview (diff for edits, command for bash); reads do not. The dispatcher also understands "allow for session" per tool name, and remembers it for the session's lifetime.

`ask_user` is an ordinary registered tool, not a special case: the dispatcher asks the interactor and leaves the answer in a thread-local slot the tool reads back. Its schema carries `prompt`, `options` and `multiple`, and the answer can be several picks, free text, or a dismissal. A dismissed question returns the sentence "the user cancelled the question" rather than an error, so the model can carry on without inventing an answer.

### Question answering, as the window does it

The dispatch thread posts the question, then blocks up to 300 s on the answer channel; a timeout is a dismissal, never an invented choice. The window shows the question in the transcript with numbered options: a digit highlights one, Enter confirms it, Escape dismisses it. So the window produces one pick or a dismissal. `multiple` and free-text answers are implemented in the agent layer and are not reachable from the window yet.

Approvals work the same way: the call card shows the preview, Enter allows once, Escape refuses, and a timeout is a refusal. The window does not offer "allow for session".

### Transcript

One card per tool call with a name, a status word and an output collapsed to its first five lines plus a count of the rest. `Transcript::toggle` can expand a card to its whole output, but nothing in the window is wired to it yet, so a card stays collapsed. Assistant text is one growing item per open turn, rendered through `rune-kit`'s markdown view in streaming mode, so an unclosed fence or emphasis never flashes its markers; a finished turn renders whole. User turns and notes are plain text. Tool cards key on call id, so a late result updates only its own card. The whole transcript column is one selection group, so a drag spans turns. The transcript has a note item, but the app does not use it yet: a startup note (a failed MCP server, a refused prompt) is written as assistant text.

### Terminal

`portable-pty` spawns the shell with the session's worktree as cwd. `libghostty-vt` parses output and holds the grid, the alternate screen and up to 10 000 rows of scrollback. The UI paints visible rows as run-length groups of cells that share a style, with the cursor drawn as a block (an inverted cell, or a block after the last glyph).

- Scrolling: the wheel moves the terminal's history under the pointer; Shift+PageUp and Shift+PageDown move it a page. A scrolled viewport stays put while output arrives, and scrolling back to offset 0 follows new output again. The dock header says "scrolled N lines" while history is on screen.
- Selection and clipboard: a drag selects cells, ⌘C copies them again, ⌘V pastes. Paste is bracketed only when the program enabled bracketed paste, and every marker inside the payload is stripped, so a paste can only end where the program expects.
- Keys: presses go through libghostty's key encoder so modifiers, arrows, function keys and page keys produce what a shell expects; a resize propagates to the PTY and the grid.
- Inline images and Kitty graphics are not drawn: the frame builder publishes cells and a cursor, and there is no image path.

No wheel handler is attached to the transcript or the sidebar, so a long transcript cannot be paged; only the terminal's grid takes a wheel.

### Files and git

A walk of the session's worktree with the `ignore` crate's rules, directories first then alphabetical. Git state comes from `git status --porcelain=v1 -z` under a 30 s timeout with `--no-optional-locks` (the app never mutates a worktree); a directory that is not a repository simply has no statuses. `gix` is not used: porcelain output is the acceptance bar, so it is the contract.

A file git reports as deleted gets a synthetic greyed-out row under its parent, so the panel never omits a change git knows about.

The left column draws those rows with their depth, icon, git state in words and colour as reinforcement. The rows are display only: no row has a press handler, so clicking a file does nothing. Selection, the read-only preview with numbered lines (capped at 256 KiB and 4 000 lines) and the "Open in Zed" action exist in `perch-files` and `perch-ui` — `FilePanel::select`, `preview_view`, `open_in_zed` — but the preview panel is never placed in the window, the toolbar button has no press, and nothing calls `open -a Zed` from the app. Directories are always drawn expanded. Branch, ahead/behind and a diff view are not implemented.

### Settings and secrets

`settings.toml` in the app data directory holds `provider_base_url`, `model`, `theme` and `mcp_servers`. Every field has a default, so an older file still loads, and the defaults are written on first run. The panel (⌘, or the gear) replaces the transcript while it is open and validates the base URL (must parse as http/https with a host, because `/chat/completions` is appended to it), the model (must be named) and the theme (one of `dark`, `light`). A rejected field keeps the panel open with the reason beside it and writes nothing. `mcp_servers` is configuration only, edited in the file; the panel does not show it.

The provider key is not part of `Settings` at all, so it has no path into the file: it goes to the keychain under service `dev.jyz.perch`, account `provider`, and the panel only shows `stored` or `missing`. An empty key on save keeps the stored one. Saving folds the new provider into the app's defaults, so the next session uses it without a restart; a session already running keeps the provider it was spawned with.

MCP OAuth tokens are keychain entries too, one JSON value per server under `mcp:<server>`.

### MCP

One host owns every configured server. A server is local (`command` + `args`, spoken to over its pipes) or remote (`url`, streamable HTTP). A remote server is always authorized: a token already in the keychain is reused, otherwise the browser flow runs — metadata discovery, dynamic client registration, PKCE, a loopback listener on `127.0.0.1:0`, then the token exchange — and the tokens are saved through the keychain store. The OAuth flow runs outside the 30 s request bound, because its bound is the one a human needs. A remote server that publishes no authorization metadata fails and is reported as failed; the HTTP transport could carry no token, but the connect path never takes that branch.

Every discovered tool becomes an ordinary `perch_tools::Tool` registered as `<server>__<tool>`, so no server can shadow a builtin or another server's tool. The registry dispatches it by name like any other, and its result is the same `ToolOutput`, which means an MCP call appears in the transcript as a normal tool card under its namespaced name.

MCP tools report `needs_approval() == false` — the user's call, because a server that needs a browser to connect is not one to prompt about on every call — and instead the host records every call as a `Note::Call` carrying the server name. The host also notes connections, failures and removals, and its lines are the transcript's words for a failure: a server that will not start is a note in every session's column, never a window that will not open.

A failed server is skipped at startup and recorded by name, so a later reconciliation can drop it instead of retrying it forever. The host is reconciled rather than rebuilt: `McpHost::reconcile` keeps a server whose name and configuration are unchanged, connects a new or changed one, and drops one that is no longer configured or whose transport has closed; `McpHost::register_into` fills a registry built from empty without reconnecting anything. `disconnect` runs on every frame, so a server that dies mid-session loses its tools at the next one rather than at the next restart.

### Plugins

A plugin is a `wasm32-wasip2` component built against `crates/perch-plugins/wit/perch-plugin.wit` (world `perch-plugin` 0.1.0). It exports `tools` (`tool-specs`, `call`) and `panels` (`panel`, `on-event`) and imports exactly one host interface, `log`. There is no host file API at all: a plugin has no import to read a file with, so the refusal is the interface rather than a runtime check.

The host owns every instance, bounds each call at 50 million fuel and 2 s, disables a plugin that traps, and refuses a tool name another tool already holds — a collision names both rather than silently overwriting a builtin. A disabled plugin's tools are removed through the registry by `release`, which is idempotent.

A panel is data: a title and sections of text, rows, named buttons and named inputs. `perch-ui` turns that model into elements with the app's own fonts, spacing and surfaces, so a plugin cannot position itself, colour itself or draw over the chrome, and hit-testing stays with the shell. `dispatch_event` serialises a session event to every loaded plugin through `on-event`; a trap there takes the same disabling path.

None of this is attached to the app yet: the app never loads a plugin directory (there is no setting for one), so no plugin tool is in the registry a session sees, no panel is drawn in the window, and no session event reaches a plugin.

### Packaging

`scripts/bundle.sh` builds `perch-app` in release into its own `target-package/` directory and stages `dist/Perch.app` by hand — one executable, one plist, one icon. The version comes from the crate, the bundle id from `APP_ID` in `main.rs`, the minimum macOS from the binary's Mach-O `minos` load command, and the icon is drawn by `scripts/make-icon.swift` then converted with `sips` and `iconutil`. Signing is ad-hoc.

`scripts/verify-bundle.sh` checks the staged bundle against those same sources of truth and prints one verdict per check.

The gap is deliberate and is not hidden: an ad-hoc signature means Gatekeeper rejects the bundle on any Mac but the one that built it, until it is signed with a Developer ID identity and notarized. Neither is implemented, and the staged binary is arm64 only.

## Errors and limits

- Provider errors surface with the exact status and body and end the turn `errored` — never a silent retry loop. A stream that ends without `[DONE]` is an error, not a short reply.
- A turn stops at 12 rounds.
- Tool output and file previews are size-capped, and the cap is stated where it bites.
- Every process wait, HTTP request and answer wait has a timeout: bash 30 s, git 30 s, open-in-Zed 30 s, one MCP request 30 s, one provider request 120 s, one answer 300 s.
- A pty outlives the shell behind it, so the dock asks the child with a non-blocking `try_wait` and says the shell exited instead of repainting a dead frame; dropping the handle sends `Shutdown` and kills the child.
- A path outside the session's worktree is refused, not clipped; symlinks are resolved before the check.

## Verification

- `scripts/gate.sh` is the gate: the 200-line file budget, `cargo fmt -p <each package> -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Provider streaming and the turn loop are tested against recorded SSE fixtures served from a local fixture server — text, tool calls, usage, a slow stream for cancellation, and a cut stream that must end errored.
- Tool assembly, approvals, deny, cancel and the question tool are tested through the dispatcher and through the app's bridge.
- Terminal tests replay recorded byte streams and assert grid contents, cursor position, styles and wide characters; a flood fixture asserts the frame build stays under 150 ms in release and a stated larger budget in a dev profile, with the split between Ghostty's Zig side and our frame builder reported.
- The window's acceptance tests go through the accessibility tree: `--tree --offline` must name `sessions`, `transcript`, `terminal` and `files`, a tab switch must move the active session, and a question must be answerable by a digit and Enter.
- `scripts/live-probe.sh` runs the real loop against free OpenRouter models and prints a verdict; it is a manual check, not part of the gate.
- `scripts/verify-bundle.sh` proves the staged bundle: plist fields match their sources, the signature verifies, the binary is Mach-O arm64, the icon decodes.

## Shipped and not shipped

Shipped: session list and tabs over JSONL logs; a streaming markdown transcript with tool cards and a single selection group; the six builtins with confinement, previews and approvals; `ask_user` in the agent layer; a worktree tree in the left column with git state in words; Ghostty terminal with scrollback, selection copy, bracketed paste and a drawn cursor; settings screen with keychain-backed key; MCP over stdio and HTTP with OAuth, namespaced tools and per-call notes; the wasm plugin host with tools, panels, events and trap isolation; the bundle scripts and the live probe.

Not shipped: an updater; a notarized or universal bundle; a DMG; loading plugins in the app, and drawing a plugin panel in the window; free-text and multi-pick answers from the window; "allow for session" in the window; expanding a tool card from the window (`toggle` exists, no press is wired); clicking a file to select it, the preview panel, and the Zed handoff (all implemented below the window, none wired into it); wheel paging of the transcript or the sidebar; cursor-level and per-cell accessibility (the tree names regions and text rows); git branch, ahead/behind and a diff view; using the transcript's note item for app notes (only MCP failures are said, and they are said as assistant text at startup); showing MCP connection and call notes in the window; pruning a dead MCP server's tools from a running window.

## Open questions

- Whether the transcript should keep full tool output on disk for replay, and for how long.
- How a plugin is discovered and enabled (a directory setting, a prompts-on-first-use rule, or a manifest in the session's worktree) once plugins attach to the app.
- Which wasmtime component-model features to permit (WASI clock and randomness, nothing else) before a plugin needs more.
- Whether the window should answer questions with multi-pick and free text by growing the composer, or by a dedicated question card.
