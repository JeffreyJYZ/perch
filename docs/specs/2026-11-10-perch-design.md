# perch — design

Date: 2026-11-10
Status: awaiting approval

## Intent

opencode is the best agent UI I use and the heaviest thing I run. perch is the harness I keep open instead: one native window, no Node runtime, no browser engine, that keeps the two things I leave opencode for — seeing the worktree and browsing its files, and a terminal in the same window — and treats plugins as part of the product from the first version.

Everything is Rust, and the UI is Rune.

## Goals

- A single Rust binary that opens a window and starts a session in under a second.
- A transcript that streams assistant text and tool calls without flicker, with selection that copies across turns.
- A real terminal: full-screen programs, colours, scrollback, correct key encoding, resize.
- A file browser that reflects the worktree and git state, previews a file read-only, and hands it to Zed on request.
- Plugins from day one: a plugin can add tools the model calls and add a side panel.
- A session is bound to a worktree that already exists on disk.

## Non-goals

- Editing files in the app. Files are read-only; edits belong to the agent or to Zed.
- Creating or deleting worktrees, branches or repositories.
- LSP, debuggers, run configurations, build panes.
- Provider-specific APIs beyond OpenAI-compatible streaming.
- Windows and Linux.
- Notarization, DMG layout and auto-update in the first version.

## Constraints

- Rune ships `dev` only, so the app pins a revision and takes breaking changes deliberately.
- The terminal core is `libghostty-vt` (MIT) through its Rust wrapper. Its C API is pre-1.0, and a vendored build needs Zig 0.17.x; a prebuilt static library avoids Zig on macOS.
- No Node, no webview, no JavaScript.
- macOS first (Apple silicon and Intel).

## Approaches considered

**One process, threads, channels — chosen.** The agent loop, provider stream, tool execution and PTY readers run on ordinary threads and hand events to the UI through channels. Rune's live values carry the latest state into each frame. This is the lightest runtime and the shortest path to the resource win that motivates the project. A misbehaving component takes the window with it.

**UI process plus a background daemon.** Crash isolation, and a headless mode later. Rejected for v1: it requires a socket protocol with versioning and reconnection before there is anything worth isolating, and it doubles the process count the project exists to reduce.

**Agent as a supervised child process.** Cheaper than a socket, but cancellation, streaming and session ownership get awkward exactly where correctness matters most.

**Terminal, three ways.** Draw our own VT grid (rejected: months of edge cases the project has no business inventing), embed a web terminal (rejected: that is the technology the project is leaving), or use Ghostty's extracted core (`libghostty-vt`) and draw its cells ourselves (chosen). It provides parsing, grid, scrollback, styles, formatting and key/mouse encoding — the parts that take years — and leaves drawing to the host, which is what a native UI wants anyway.

**Plugins, four ways.** Luau (small and in the Rune tree, but adds a second language to an all-Rust project), native Rust dylibs (fast, but a bad plugin can corrupt the app and every plugin needs a rebuild per OS), separate processes (language-agnostic and isolated, but heavier per call and no UI access without inventing a protocol), Wasm components via wasmtime (chosen: sandboxed, written in Rust, one ABI, hot-loadable).

**Sessions and worktrees.** The app creates worktrees (convenient, but it then owns cleanup, locking and uncommitted work), no worktree concept (simplest, and two sessions overwrite each other), or a session binds to a worktree that already exists (chosen: least code, least chance of losing work).

## Architecture

One binary, one window, four teams of threads.

- **UI (Rune, main thread).** Draws the frame, owns focus, keybindings and panels. Reads the latest state through live values; never blocks on IO.
- **Agent.** Owns the turn: message list, provider request, streamed deltas, tool calls, approvals, cancellation, token counts. Emits events; never touches UI state directly.
- **Tools.** Execute in worker threads; each returns a result plus the diff or output the approval view needs. Bounded by timeouts, never by a bare blocking call.
- **Terminal.** One thread per terminal tab owns both the PTY reader and the Ghostty grid, because `libghostty-vt` is neither `Send` nor `Sync`; it publishes plain frame snapshots that the UI paints.
- **Plugins.** wasmtime instances on their own thread; calls into them are bounded, and a trap disables that plugin without taking the app down.

Events are typed enums on a channel. The UI keeps the last known value per panel; nothing re-derives state from scratch per frame.

Persistence: one JSONL file per session under the app's data directory — messages, tool results, token counts, the worktree path and the model. Append-only, so a crash loses at most the current turn. Settings live in one TOML file; provider keys live in the macOS keychain.

Sessions: the first version carries a session list and tabs, not one session per window. The left column lists sessions with their worktree and branch; a tab strip switches between them; each tab owns its transcript, its terminal tabs and its file tree root. Switching tabs is a state swap, never a reload from disk, and the list is read from the JSONL headers at start so the window opens with history.

## Components

### Agent loop

Turn state machine: idle → requesting → streaming → tool calls → waiting for approval → next request → done, plus cancelled and errored. The provider client is one OpenAI-compatible streaming implementation over SSE, reading `choices[].delta`, tool-call deltas and usage. Base URL, key and model come from settings, so OpenRouter, Command Code and local servers all work with no extra code.

Streaming rules that matter: deltas append to a message buffer, markdown re-parses the tail so half-written markup never flashes; a turn's scroll follows the bottom only while the user is at the bottom; cancellation is cooperative and always ends in a definite state.

### Tools

`read`, `write`, `edit`, `bash`, `list`, `grep` — the smallest set that lets an agent work, each with a stated contract for paths, limits and output size.

Permissions: `write`, `edit` and `bash` require approval with a preview (diff for edits, command for bash), with an allow-for-session option. Plain reads do not. Every tool result is truncated to a stated budget before it reaches the model, and the full output stays in the UI.

### Transcript

Markdown in streaming mode; one card per tool call with a one-line summary, expandable output, and a status. Selection is grouped so a drag spans turns and copy yields the whole conversation. Assistant text, user turns and tool cards are visually distinct without colour-only signalling.

### Terminal

`portable-pty` spawns the shell with the session's worktree as cwd. `libghostty-vt` parses output and holds the grid and scrollback. The UI paints visible rows: run-length grouping of cells that share a style, cursor drawn separately, selection mapping cell coordinates to text. Key presses are encoded by libghostty's key encoder so modified keys, function keys and paste behave; resize propagates to the PTY and resets the grid size. Kitty graphics and inline images are out of scope for v1 (parsed, not drawn).

### Files

A walk of the session's worktree using the ignore crate's rules, git status from `gix` (falling back to `git2` only if needed), Material Symbols shipped as a bundled font for file-type icons. Clicking a file opens a read-only preview panel with syntax colouring and line numbers; a toolbar button opens the same path in Zed via `open -a` plus a path argument. Directories expand in place. Ignored files are hidden by default with a toggle.

### Git

Branch, ahead/behind, and per-file status in the tree; a diff view shared with the edit-approval view. No commit, stage or push in v1.

### Plugins

wasmtime, component model, WIT interface versioned from the first release. Capabilities:

- `add-tool`: name, description, JSON schema, and an implementation that receives arguments and returns text. The model sees it exactly like a built-in tool.
- `add-panel`: a declarative description — title, rows, text, buttons, inputs — rendered by the host into the side column. No custom drawing, so a plugin cannot fake the app's chrome or crash the renderer.
- `events`: a read-only stream of session events (turn started, tool called, turn finished) so a plugin can react without touching internals.

The host mediates all IO: a plugin asks, the host decides. A plugin that traps is disabled and reported in the UI, and its tools disappear from the registry.

## Errors and limits

- Provider errors surface as a message in the transcript with the exact status and body, and the turn ends in `errored` — never a silent retry loop.
- Provider streams have an idle timeout; cancellation and timeouts both produce one terminal event.
- Tool output and file previews are size-capped; the cap is stated in the UI when hit.
- Terminal readers never block on a dead child; teardown is bounded and a survivor is reaped on its own thread.
- A path outside the session's worktree is refused, not clipped.

## Verification

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` for the agent, tools, plugin host and terminal state.
- A scripted provider fixture (a local HTTP server replaying recorded SSE) so the loop and streaming are tested without network or cost.
- Terminal: replay recorded byte streams and assert grid contents, cursor position and styles; a flood test asserting frame time stays under budget while `cat` of a large file streams.
- UI: Rune's accessibility tree dump asserted for the main screens, so "it renders" is proved by a tree, not a screenshot alone.
- Perf acceptance, measured not assumed: 10k-row transcript scroll, terminal flood, and a 200-file tree expanding, each within a stated frame budget on this machine.

## Milestones

1. Skeleton: window, three columns, settings, session file, palette. Acceptance: app opens to an empty session, settings persist.
2. Terminal spike: PTY plus Ghostty grid, one tab, drawn rows. Acceptance: interactive shell, `vim` renders correctly, flood test within budget. This goes second because it is the riskiest drawing path.
3. Agent loop: streaming client, transcript, cancel, token counts, fixture tests. Acceptance: a recorded conversation replays and streams correctly.
4. Tools and approvals: the six tools, diff and command previews, allow-for-session. Acceptance: an agent completes a small edit task end to end.
5. Files and git: tree, icons, status colouring, preview, open in Zed. Acceptance: a worktree browses correctly and git state matches `git status`.
6. Wasm tools: registry, host IO mediation, trap isolation.
7. Wasm panels: declarative panel host.
8. Polish: keybindings, themes, scrollback edge cases, packaging.

## Open questions

- Session switching and tabs in v1, or one session per window until later.
- Whether the transcript should keep full tool output on disk for replay, and for how long.
- Which wasmtime component-model features to permit (WASI clock and randomness, nothing else) before any plugin exists to need more.
