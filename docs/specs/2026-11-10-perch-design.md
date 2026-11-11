# perch — design

Date: 2026-11-10
Status: shipped as 0.1.0 — thirteen crates, the core harness exists and is gated; the gaps are listed under "Shipped and not shipped". This document describes the code that is in the tree, not the plan that preceded it.

## Intent

opencode is the best agent UI I use and the heaviest thing I run. perch is the harness I keep open instead: one native window, no Node runtime, no browser engine, that keeps the two things I leave opencode for — seeing the worktree and browsing its files, and a terminal in the same window — and treats plugins as part of the product from the first version.

Everything is Rust, and the UI is Rune.

## Goals

- A single Rust binary that opens a window and starts a session in under a second.
- A transcript that streams assistant text and tool calls, with one card per call, expandable cards and selection that spans the column.
- A real terminal: full-screen programs, colours, scrollback, correct key encoding, resize, bracketed paste and selection copy.
- A file browser that reflects the worktree and git state, opens a file in a tab, previews it read-only, and hands it to Zed on request.
- A model that can ask the user a question with options (one or several) and a free-text answer, and pause the turn until it is answered.
- MCP servers as tool sources: local stdio, remote HTTP, and the OAuth flow, with MCP tools allowed to run without an approval prompt but every call recorded.
- Plugins from day one: a plugin can add tools the model calls, declare a side panel, and hear session events.
- A session is bound to a worktree that already exists on disk.
- One `settings.toml` for values, one `0600` secrets file for the provider key, the keychain for MCP tokens, and one append-only JSONL log per session.

## Non-goals

- Editing files in the app. Files are read-only; edits belong to the agent or to Zed.
- Creating or deleting worktrees, branches or repositories.
- LSP, debuggers, run configurations, build panes.
- Provider-specific APIs beyond OpenAI-compatible streaming.
- Windows and Linux.
- Notarization, DMG layout, and an updater that installs itself.

## Constraints as they turned out

- Rune ships `dev` only, so `perch-ui` and `perch-app` depend on the local checkout at `../../experiments/rune/crates/{rune,rune-kit}`. There is no registry pin to float.
- The terminal core is `libghostty-vt` 0.2.2 (MIT) through its Rust wrapper, pinned exactly. Its vendored Ghostty is built from source, so **Zig is required**: the vendored tree sets `minimum_zig_version = "0.15.2"` and refuses 0.17.1 at configure time. `scripts/zig-env.sh` sources the keg-only 0.15.2 and, on macOS 27 where the SDK's libSystem stubs list only `arm64e`, writes a cached SDK shim into `.zig-env/`.
- No Node, no webview, no JavaScript.
- macOS only. The bundle script stages an `arm64` binary; there is no Intel build or universal artefact, so the original "Apple silicon and Intel" reading is not true of what ships.
- Provider secrets are a `0600` file, not the keychain. The keychain grants access per binary, and every ad-hoc-signed local build is a new binary, so it answered "not stored" to the app that needed the key and turns went out with no authorization header. MCP OAuth tokens stay in the keychain because the OAuth client writes through the keychain credential store and its tokens are not the provider's.

## Approaches considered

**One process, threads, channels — chosen.** The agent loop, provider stream, tool execution, dispatch thread, plugin worker, loader, watcher and PTY readers run on ordinary threads and hand events to the UI through channels. Rune's live values carry the latest state into each frame. A misbehaving component takes the window with it.

**UI process plus a background daemon.** Crash isolation, and a headless mode later. Rejected: it requires a socket protocol with versioning and reconnection before there is anything worth isolating.

**Agent as a supervised child process.** Cheaper than a socket, but cancellation, streaming and session ownership get awkward exactly where correctness matters most.

**Terminal, three ways.** Draw our own VT grid (rejected: months of edge cases), embed a web terminal (rejected: that is the technology the project is leaving), or use Ghostty's extracted core (`libghostty-vt`) and draw its cells ourselves (chosen). It provides parsing, grid, scrollback, styles, formatting and key/mouse encoding, and leaves drawing to the host.

**Plugins, four ways.** Luau (adds a second language to an all-Rust project), native Rust dylibs (a bad plugin can corrupt the app, and every plugin needs a rebuild per OS), separate processes (language-agnostic and isolated, but heavier per call and no UI access without inventing a protocol), Wasm components via wasmtime (chosen: sandboxed, written in Rust, one ABI, hot-loadable).

**Sessions and worktrees.** Creating worktrees makes the app own cleanup, locking and uncommitted work; no worktree concept lets two sessions overwrite each other; a session binds to an existing worktree (chosen).

**Secrets, two ways.** The keychain (per-binary approval, so it fails for local builds — rejected for the provider key) and one `0600` JSON file written through a temporary (chosen, matching OpenCode's `auth.json`). MCP tokens keep the keychain because they arrive through the OAuth client's credential store.

## Architecture

One binary, one window, and one thread per team.

- **UI (Rune, main thread).** Draws the frame, owns focus, keybindings and panes. Reads the latest state through live values; never blocks on IO. The window is 1280×820 points, a titled window rather than full-size content.
- **Agent.** Owns the turn: message list, provider request, streamed deltas, tool-call assembly, rounds, cancellation, token counts. Emits typed events; never touches UI state directly.
- **Dispatch.** One thread per session runs every tool call, asks the user for approvals and answers, and sends the result back to the turn. A turn that is waiting on a person blocks only that thread.
- **Terminal.** One thread per terminal tab owns both the PTY reader and the Ghostty grid, because `libghostty-vt` is neither `Send` nor `Sync`; it publishes plain `Frame` snapshots that the UI paints.
- **Plugin worker.** One thread owns the whole plugin host, so a guest call or a trap never runs on the frame that draws.
- **Loader.** Builds a tool registry from the built-ins, the plugin directory and the MCP servers, and swaps it in as one value; a slow server costs the tools their seconds and never a frame.
- **MCP host.** One Tokio runtime, because `rmcp` is async and the tool trait is not. Calls enter it with `block_on` from the caller's thread.
- **Update worker.** One thread reads the manifest, and one job stages a verified archive; the frame only reads the status it last published.
- **Title worker.** One small provider request after a session's first turn names it; the answer lands off the frame and off the turn, and never over a title a person wrote.
- **Watcher.** One thread turns a filesystem burst on `settings.toml` or the plugin directory into one debounced signal.

Events are typed enums on a channel. The UI keeps the last known value per region; nothing re-derives state from scratch per frame.

Two crates are built and tested but not registered by the app: `perch-web` (the read-only `webfetch` and `websearch` tools, behind a `Fetcher` seam so tests never touch the network) and `perch-board` (the project board's append-only log and its post/read/wait tools). Neither is offered to a model yet.

Persistence: one JSONL file per session, `<data dir>/<id>.jsonl`, whose first line is the session header (id, title, worktree, branch, model, created) and whose later lines are user and assistant text, tool calls, tool results and turn endings. Appends write whole lines, so a crash costs at most the line being written, and a torn line is dropped on read. Settings live in one TOML file; the provider key lives in `secrets.json` (`0600`); MCP OAuth tokens live in the keychain under `mcp:<server>`.

Sessions: the left column lists sessions and the strip switches them. The active session owns its transcript, its terminal tabs and its file tree root. Switching is a state swap, never a reload from disk; the list is read from the JSONL headers at start so the window opens with history. A session whose worktree has gone refuses new turns and says why in its own transcript. `branch` is written empty today: the app does not read the worktree's git branch, so the session row says "no branch".

## Components

Layout: one column to the left, full window height, holding the session list above the file tree; the centre holds one tab strip and the pane it names — the transcript with the prompt under it, a file's preview, or the settings form; the terminal dock sits under the centre only, so it belongs to the session it shows; and a right-hand column exists only when there is a plugin panel, a word about a plugin, or an update offer to draw. State is written in words — "waiting", "done", "failed", "approval needed", "missing", "stored" — never a colour alone.

### Panes and tabs

One value decides the centre: `Pane::{Session, File(PathBuf), Settings}`. The strip draws the sessions (a speech-bubble glyph), one tab per opened file (a page glyph, closable), and `Settings` last (a gear); the one whose pane is showing is marked active, and nothing else decides what is drawn.

- A press on a file row opens or focuses a tab for that path and makes it the pane; two files of the same name in different directories get a tab each, and reopening a file focuses the tab it already has.
- Closing the shown file tab returns to the session and keeps the tree's selection, so the same file is one press from coming back. Session tabs carry no close control: a session is not deleted from the strip. **Caveat:** the close chip is invisible until the pointer is over its tab, so a press whose coordinates were taken from the pre-hover tree addresses whatever sits there instead — which is how a headless test of the last file tab's close can end on `Settings`. Rune fires `on_press` at release against the path captured at press time, so this is not the release landing on a reflowed tab; the press path simply has not been pinned from a hovered frame. `Shell::close_tab` itself returns to the session correctly, which is why the wiring test drives it directly.
- `Escape` on the preview leaves it, as the header's button does; the settings pane is left the same way.
- Switching session resets the pane and the file tabs, because the tabs name files in a tree that was just replaced.
- The gear that used to sit beside the strip is gone: `Settings` is a tab, and ⌘, is its chord.

### Commands

A composer text that starts with `/` is a command, not a prompt. Typing `/` in an empty composer opens a filtered list of them; a row writes the command's typed form into the field. Every entry has a real effect, and one whose precondition is missing says what is missing in the transcript instead of doing nothing:

- `/new` starts a session on the worktree already open; `/fork` copies this conversation into a new session; `/rename <title>` writes the title the list and the log carry; `/sessions` moves the keyboard into the session list (Up/Down move, Escape returns).
- `/model [id]` opens the model picker, or sets the id given; `/compact` asks the session to compact now.
- `/skills` lists the skills this session can load and `/skill <name>` loads one's body into the prompt.
- `/theme [light|dark]` switches the kit and saves it when an app is attached (a window with no app switches for the frame only and says so); `/terminal` opens another terminal tab; `/editor` hands the selected file to Zed.
- `/export` writes the transcript as plain text to `<data>/export-<id>.txt`; `/clean` empties the column (the log is untouched); `/help` prints every command; `/quit` closes the window.
- Anything else, including a slash with no name, is answered in words.

⌘K opens the command palette: the same command table, never a copy, plus the four window actions that have no slash name (Settings, New session, Toggle dock, Close tab). Typing filters it, Up/Down move, Enter takes the row, Escape closes it.

⌘P opens the file finder: a full-window layer over whatever pane is showing, so Escape returns to it untouched. The worktree is read once per open (the same ignore-aware walk as the tree, skipping `.git`, `target` and `node_modules`), capped at 20 000 files with the cap named. Matching is a subsequence over the whole path, with one dropped letter forgiven on needles of five or more, ordered by missed letters, then a name-prefix hit, then length, then path; twelve rows are drawn and the overlay says how many matched. Enter opens the pick through the same `Shell::open_file` a tree-row press uses, so it lands in a file tab.

A session is named by itself once: after its first turn, one small request (six words at most, in the question's language) replaces the folder name, only while the title is still that folder name, so `/rename` always wins and a session is never named twice. A failed title request leaves the folder name and writes one line in the log, never a word in the transcript. A session's row carries a delete control: the first press arms it and says so, the second removes the session and deletes its log.

### Agent loop

Turn state machine: idle → requesting → streaming → tool calls → waiting for approval or an answer → next request → done, plus cancelled and errored. The provider client is one OpenAI-compatible streaming implementation over SSE, reading `choices[].delta`, tool-call deltas and usage, with a 120 s budget per request. Base URL, key and model come from settings, so OpenRouter, Command Code and local servers all work with no extra code.

A turn is a short conversation, not one request: every call is answered and the next request carries the assistant's call and the tool messages beside it. It ends when the model answers without calling anything, at `MAX_ROUNDS` (12), on cancellation or on an error. Tool-call fragments are assembled per index and parsed once at the end of the stream; a parse failure ends the turn errored rather than yielding half an argument set. Cancellation is cooperative and always produces exactly one `TurnEnd`.

The model list is a separate, bounded request (10 s) against the provider's listing URL, returning each id with the context window the provider states for it. The picker never waits on a turn and a provider that lists nothing still accepts a typed id; the context window is what the meter divides into.

### Tools

`read`, `write`, `edit`, `patch`, `bash`, `jobs`, `list`, `grep`, `glob` — nine built-ins, registered rather than enumerated: `ToolRegistry::call` dispatches by name only, and `specs()` is computed from what is registered, so a plugin or MCP tool takes the same path.

Three more tools are registered by the app's build, in an order that is the safety property: `skill`, then `task`, then `execute`. `task` and `execute` each take a copy of the registry as it is at that moment, so a child never sees `task` or `execute` (no grandchildren, no programs) and a program sees both.

- `skill` (perch-agent) finds `SKILL.md` directories under `~/.config/perch/skills` first, then `.perch/skills` from the filesystem root down to the worktree, nearest last so a project overrides a machine-wide skill of the same name. Front matter carries the name and description; the body is capped at 64 KiB with the cut named. `/skills` lists them and `/skill <name>` loads one into the prompt.
- `task` (perch-agent) runs one child turn from a self-contained `description` and `prompt`, and answers `task "<label>" -> done[ in N rounds]` plus the child's final text. The child is handed only tools with `needs_approval() == false` — so it can read, list, glob and grep, never write, edit, patch or run a shell — a registry that already holds `task` is refused, the parent's cancel stops it, a prompt over 64 KiB is refused and an answer over 64 KiB is cut with the cut named.
- `execute` (perch-codemode) is Code Mode: a real JavaScript program (boa) with `tools.call(name, args)`, `tools.list()` and `print` bound, so several calls and the work between them cost one tool call and only what the program prints reaches the transcript. The registry is untouched and a tool that needs approval cannot be called from a program — it throws, asking the model to call it directly so a person can approve it. Each execution gets 30 s, 32 tool calls and 64 KiB of output, and the error names whichever ran out.

Contract: `bash` runs `sh -c` in the worktree under a 30 s timeout with both streams captured and output truncated at 16 KiB; `background: true` starts the command as a job and returns at once. Jobs live per worktree root (so two sessions never see each other's), are capped at 16 concurrent, run in their own process group, write one log file each, and are read or killed through `jobs` (`list`, `read` with a tail of 40 lines by default and 200 at most, `kill`). `read` caps at 64 KiB and numbers lines; `list` stops at 512 entries; `grep` launches `rg` under the same 30 s timeout and names a missing `rg` rather than returning empty; `glob` walks ignore-aware, lets `**` cross directories, sorts by path, caps at 512 and refuses a pattern that escapes the worktree; `edit` replaces one exact occurrence and returns a unified diff; `patch` applies a unified diff across several files all-or-nothing, names the file and hunk on failure, and creates or deletes through `/dev/null`. `preview` is computed without running the tool, and a preview that fails denies the call.

Permissions: `write`, `edit`, `patch` and `bash` require approval with a preview (diff for edits and patches, command for bash); `read`, `list`, `grep`, `glob` and `jobs` do not, because a job can only be started through `bash`, which does. The dispatcher understands "allow for session" per tool name for the session's lifetime.

A saved `Rules` file outlives that: one JSON object per line (`{"tool":"bash","pattern":"cargo test","decision":"allow"}`), consulted before the person is asked, with an allow never widened beyond its pattern and a path pattern resolved through the same confinement the tool uses. The dispatcher supports it (`Dispatcher::with_rules`), but the app builds `Dispatcher::new()`, so no rules file is read yet.

Project instructions are implemented in the same crate (`instructions::for_worktree`): `AGENTS.md` and `CLAUDE.md` from the filesystem root down to the worktree, outermost first so the nearest file has the last word, 32 KiB per file and 96 KiB in total, a symlink followed only inside the worktree. The app's system prompt is still the fixed sentence in `app/mod.rs`, so those files are not sent yet.

`ask_user` is an ordinary registered tool, not a special case: the dispatcher asks the interactor and leaves the answer in a thread-local slot the tool reads back. Its schema carries `prompt`, `options` and `multiple`, and the answer can be several picks, free text, or a dismissal. A dismissed question returns the sentence "the user cancelled the question" rather than an error, so the model can carry on without inventing an answer.

### Question answering, as the window does it

The dispatch thread posts the question, then blocks up to 300 s on the answer channel; a timeout is a dismissal, never an invented choice. The window shows the question in the transcript with numbered options: a digit toggles that option, Enter confirms the set, Escape dismisses it, and `t` opens a field for a free-text answer. A single-choice question keeps one pick; a `multiple` one sends every option toggled on, in the order it was picked. Enter with nothing picked is refused in words and the question stays up; an empty free-text answer is refused the same way.

Approvals work the same way: the call card shows the preview, Enter allows once, ⌘⏎ allows the tool for the rest of the session, Escape refuses, and a timeout is a refusal.

### Transcript

One card per tool call with a name, a status word and an output collapsed to its first five lines plus a count of the rest. A press focuses a card and Enter or Space toggles it between collapsed and whole; the focus is what makes the key a press on that card rather than on anything beneath it. Assistant text is one growing item per open turn, rendered through `rune-kit`'s markdown view in streaming mode, so an unclosed fence or emphasis never flashes its markers; a finished turn renders whole. User turns are plain text; a cancelled or failed turn is a note in the column's quieter voice, and each turn ends with a one-line summary of calls, failures and tokens. The whole transcript column is one selection group, so a drag spans turns.

The column scrolls in one viewport that takes the wheel: scrolled up it stays where the reader left it while output arrives, and back at the bottom it follows again. A header says "scrolled N lines" when it is not at the bottom.

### Context meter

Beside the composer is one line of numbers: the tokens the last turn reported, over the context window the provider states for the chosen model. Nothing is shown before the first turn, because a meter that reads `0%` has measured nothing, and a provider that states no window leaves the share out rather than inventing a denominator. Tokens are written as `999`, `1.2K` or `1.0M`, with the thousand and million cut-offs OpenCode uses, so the same conversation reads the same in both harnesses. When there is room to say it, the same row names `/compact` as the way to shrink the conversation.

The listing request is where the denominator comes from: each `ListedModel` carries the id and the context window however the provider spells it (`context_length`, `context_window`, or a top-level provider's), and the picker works from the same list.

### Compaction

A provider that refuses a conversation as too long has answered a question about its size, so the same request is never sent again. The oldest exchanges are dropped, the newest kept, and the request re-sent at 0.85, 0.70, 0.50, then 0.35 of the refused size; a refusal at the last step ends the turn with the provider's error. Each shrink emits `Compacted { dropped }` and the transcript notes it, so a column that loses history says so. `/compact` asks for the same thing between turns: it travels the agent's queue, emits no `TurnEnd`, holds a prompt sent behind it, and answers "nothing to compact" when there is no older exchange to drop.

### Terminal

`portable-pty` spawns the shell with the session's worktree as cwd. `libghostty-vt` parses output and holds the grid, the alternate screen and up to 10 000 rows of scrollback. The UI paints visible rows as run-length groups of cells that share a style, with the cursor drawn as a block (an inverted cell, or a block after the last glyph).

- Scrolling: the wheel moves the terminal's history under the pointer; Shift+PageUp and Shift+PageDown move it a page. A scrolled viewport stays put while output arrives, and scrolling back to offset 0 follows new output again. The dock header says "scrolled N lines" while history is on screen.
- Selection and clipboard: a drag selects cells, ⌘C copies them again, ⌘V pastes. Paste is bracketed only when the program enabled bracketed paste, and every marker inside the payload is stripped, so a paste can only end where the program expects.
- Keys: presses go through libghostty's key encoder so modifiers, arrows, function keys and page keys produce what a shell expects; a resize propagates to the PTY and the grid.
- Inline images and Kitty graphics are not drawn: the frame builder publishes cells and a cursor, and there is no image path.

The grid is exposed to the accessibility tree as rows of text with the caret and the selection named on the row that carries them, so a screen reader hears where the cursor is without a cell for every character.

### Files and git

A walk of the session's worktree with the `ignore` crate's rules, directories first then alphabetical. Git state comes from `git status --porcelain=v1 -z` under a 30 s timeout with `--no-optional-locks` (the app never mutates a worktree); a directory that is not a repository simply has no statuses. `gix` is not used: porcelain output is the acceptance bar, so it is the contract.

A file git reports as deleted gets a synthetic greyed-out row under its parent, so the panel never omits a change git knows about.

The left column draws those rows with their depth, icon, git state in words and colour as reinforcement, and nothing else: the tree scrolls in its own box and never draws a file's contents. A press on a row opens that file's tab and puts the read-only preview in the centre column at full width — monospace, one numbered line per row in a fixed gutter, no soft wrap — while the transcript keeps streaming out of sight. The preview's header names the path and its language, offers "Open in Zed" (whose press hands the path to the app's editor through a small `Editor` seam, and says a refusal in words), and says that Escape returns to the transcript with the file still selected. Lines are capped at 256 KiB and 4 000 lines, and the header states the cap when it bites. A file git reports deleted is named as deleted instead of previewed. Directories are always drawn expanded. Branch, ahead/behind and a diff view are not implemented.

### Settings and secrets

`settings.toml` in the app data directory holds `provider_base_url`, `model`, `theme`, `update_check` and `mcp_servers`. Every field has a default, so an older file still loads, and the defaults are written on first run. The `Settings` tab (or ⌘,) replaces the transcript while it is open and validates the base URL (must parse as http/https with a host, because `/chat/completions` is appended to it), the model (must be named), the theme (one of `dark`, `light`) and the update check (empty, or an http/https URL with a host). A rejected field keeps the pane open with the reason beside it and writes nothing. `mcp_servers` is configuration only, edited in the file; the pane does not show it.

The model field is a picker, not a bare text box: **Refresh** asks the provider for its ids (10 s bound) and lists them; a typed id still works, because a provider need not list its models. The same listing carries each model's stated context window, which is the meter's denominator. The composer carries the current session's model and a **Change** press, which opens the settings pane in a session-only mode — the pick applies to that session and does not rewrite the saved default. Saving from the full pane makes the model both the session's and the default, and a save folds the new provider into the app's defaults so the next session uses it without a restart.

The provider key is not part of `Settings` at all, so it has no path into the file: it goes to `secrets.json` in the data directory, created `0600` from its first byte and replaced through a temporary file, under the account `provider`. The pane only shows `stored` or `missing`; an empty key on save keeps the stored one. The environment is the fallback: `PERCH_API_KEY`, `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY` for the key, and `PERCH_BASE_URL`, `OPENROUTER_BASE_URL`, `OPENAI_BASE_URL` when the saved address is empty; a saved value wins, and no address at all is a refused turn rather than a guessed endpoint. A save also hands the new address and key to every open session, so the one on screen starts using them at its next request instead of answering `401` with a provider that was just fixed.

MCP OAuth tokens are the one thing still in the keychain: service `dev.jyz.perch`, one JSON value per server under `mcp:<server>`.

### Updates

`Settings::update_check` is the release manifest. Empty is the default and means no request is ever made; a non-empty value is validated like the provider address, and the app starts one check at launch. A save that changes the URL restarts that check, and an empty one stops it, so the pane's field applies with no restart. The check, the download and the verification run on a thread of their own; the frame only reads the status the thread last published, so no frame waits on a socket.

A release newer than this build becomes one line in the right-hand column: "update X available" with the notes flattened to one line. That panel belongs to the app, not a plugin — its owner is `perch/update`, which a wasm stem can never be — and it declares one control, `stage`. A press travels the same road as a plugin control, through `Panels::panel_action`, and the app answers it by queueing the download instead of reaching a guest; `AppState::panel_action` recognises the owner and refuses a press that no standing offer can address.

The download is verified against the manifest's SHA-256 and written to `<data>/updates/perch-<version>.zip`. Installing is deliberately not wired: the bundle being replaced is the one this process is running from, so the person quits perch and unpacks the staged archive over it. The crate's boundary is check and stage, never install.

### MCP

One host owns every configured server. A server is local (`command` + `args`, spoken to over its pipes) or remote (`url`, streamable HTTP). A remote server is always authorized: a token already in the keychain is reused, otherwise the browser flow runs — metadata discovery, dynamic client registration, PKCE, a loopback listener on `127.0.0.1:0`, then the token exchange — and the tokens are saved through the keychain store. The OAuth flow runs outside the 30 s request bound, because its bound is the one a human needs. A remote server that publishes no authorization metadata fails and is reported as failed; the HTTP transport could carry no token, but the connect path never takes that branch.

Every discovered tool becomes an ordinary `perch_tools::Tool` registered as `<server>__<tool>`, so no server can shadow a builtin or another server's tool. The registry dispatches it by name like any other, and its result is the same `ToolOutput`, which means an MCP call appears in the transcript as a normal tool card under its namespaced name.

MCP tools report `needs_approval() == false` — the user's call, because a server that needs a browser to connect is not one to prompt about on every call — and instead the host records every call as a `Note::Call` carrying the server name. The host also notes connections, failures and removals. What the window draws from all that is the failures: one line each, in the right-hand column. The call and connection notes are recorded and not drawn anywhere, so a successful MCP call is visible only as its tool card.

The host is reconciled rather than rebuilt: `McpHost::reconcile` keeps a server whose name and configuration are unchanged, connects a new or changed one, and drops one that is no longer configured or whose transport has closed; `McpHost::register_into` fills a registry built from empty without reconnecting anything. `Sources::settle` runs whenever the worker publishes a snapshot, so a server that dies mid-session loses its tools at the next frame rather than at the next restart. A failed server is recorded by name, so a later reconciliation can drop it instead of retrying it forever.

### Plugins

A plugin is a `wasm32-wasip2` component built against `crates/perch-plugins/wit/perch-plugin.wit` (world `perch-plugin` 0.1.0). It exports `tools` (`tool-specs`, `call`), `panels` (`panel`, `on-event`) and `actions` (`action`) and imports exactly one host interface, `log`. There is no host file API at all: a plugin has no import to read a file with, so the refusal is the interface rather than a runtime check.

The host owns every instance, bounds each call at 50 million fuel and 2 s, disables a plugin that traps, and refuses a tool name another tool already holds — a collision names both rather than silently overwriting a builtin. A disabled plugin's tools are removed through the registry by `release`, which is idempotent. One directory of `*.wasm` is loaded: `<data>/plugins`, or the directory `--plugins` names. A file that will not load is one line in the right-hand column, never a failure to open the window.

A panel is data: a title and sections of text, rows, named buttons and named inputs. `perch-ui` turns that model into elements with the app's own fonts, spacing and surfaces, so a plugin cannot position itself, colour itself or draw over the chrome, and hit-testing stays with the shell. `dispatch_event` serialises a session event to every loaded plugin through `on-event`; a trap there takes the same disabling path.

A control in a panel is reported back through `actions`: one function, `action(id, value)`, where `id` is the control's name and `value` is the text an input was submitted with, or nothing for a button. `PluginHost::panel_action` reads the plugin's panel first and refuses an id it does not declare, so a press for a control that has gone is words rather than a call the guest would ignore.

The window presses through `Panels::panel_action`: a button's press and an input's submit both reach the app, which checks the drawn panel and hands the press to the worker that owns the plugin host. The panel is re-read into the next snapshot, so a plugin's answer is drawn a pump later and the window never waits on a guest. `AppState` refuses a press the drawn panel cannot address, and says it in the column; a press that traps on the worker's thread disables the plugin exactly as a trapping tool does and its words join the column.

### Packaging

`scripts/bundle.sh` builds `perch-app` in release into `target-package/` and stages `dist/Perch.app` by hand — one executable, one plist, one icon. The version comes from the crate, the bundle id from `APP_ID` under `crates/perch-app/src`, the minimum macOS from the binary's Mach-O `minos` load command, and the icon is drawn by `scripts/make-icon.swift` then converted with `sips` and `iconutil`. Signing is ad-hoc.

`scripts/verify-bundle.sh` checks the staged bundle against those same sources of truth and prints one verdict per check.

The gap is deliberate and is not hidden: an ad-hoc signature means Gatekeeper rejects the bundle on any Mac but the one that built it, until it is signed with a Developer ID identity and notarized. Neither is implemented, and the staged binary is arm64 only.

## Errors and limits

- Provider errors surface with the exact status and body and end the turn `errored` — never a silent retry loop. A stream that ends without `[DONE]` is an error, not a short reply. The one exception is a refusal for length: a 400 whose body says the context is too long is answered with a smaller request (85%, 70%, 50%, then 35% of the size refused, dropping the oldest exchanges and never the one being answered), and the transcript is told each time.
- A turn stops at 12 rounds.
- Tool output and file previews are size-capped, and the cap is stated where it bites.
- Every process wait, HTTP request and answer wait has a timeout: bash 30 s, git 30 s, open-in-Zed 30 s, one MCP request 30 s, one provider request 120 s, one model listing 10 s, one update request 30 s, one answer 300 s.
- A pty outlives the shell behind it, so the dock asks the child with a non-blocking `try_wait` and says the shell exited instead of repainting a dead frame; dropping the handle sends `Shutdown` and kills the child.
- A path outside the session's worktree is refused, not clipped; symlinks are resolved before the check.
- A plugin, an MCP server or an update check that fails is a line in the right-hand column, never a window that will not open.

## Verification

- `scripts/gate.sh` is the gate: the 200-line file budget, `cargo fmt -p <each workspace member> -- --check` with names read from `cargo metadata`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Provider streaming and the turn loop are tested against recorded SSE fixtures served from a local fixture server — text, tool calls, usage, a slow stream for cancellation, and a cut stream that must end errored. Model listing is tested through its transport.
- Tool assembly, approvals, deny, cancel, the question tool's picks and free text, and allow-for-session are tested through the dispatcher and through the app's bridge. Each of the nine builtins has its own suite: confinement and truncation, `bash` background jobs capped and killable, `patch`'s all-or-nothing apply, `glob`'s ignore rules and refusal to escape the root.
- `task` is tested against a fake delegate: one child turn, only approval-free tools handed over, no `task` in the child's registry, the parent's cancel stopping it, and both 64 KiB limits. `execute` is tested with a program that calls a tool, one that throws, the approval refusal, and each of the three budgets; a runaway loop is proved to stop under a hard thread timeout. `skill` is tested for discovery order, a name a project overrides, a missing front matter file, and the 64 KiB cut.
- The finder and the palette are tested through a laid-out window: ⌘P opens the overlay over the pane and Escape returns, ranking places a prefix hit first, the drawn rows and the total are said, Enter opens the file tab, and ⌘K runs a command and a window action.
- Compaction is tested against the fixture server's too-long response: the shares shrink in order, each step is announced, and the turn ends with a clear error when the last step is still refused. The meter's words and its "nothing measured yet" state are unit tests, and the commands are tested through the window: a slash runs its effect, a missing precondition is said in words, and unknown text stays a prompt.
- Auto-title is tested through its transport: one request per session, only while the title is still the folder name, and a failure leaves the folder name and writes no transcript line.
- MCP is tested against an in-repo fixture server: stdio tools, namespacing, a killed server losing its tools, and reconciliation across a configuration change. There is no OAuth server in the tests.
- Plugins are tested with checked-in components: a tool registers and runs, a trap disables it, a name collision is refused, a panel's model flattens and draws, and a press reaches the guest.
- The updater is tested through its transport: a manifest offer, a newer version, a checksum mismatch leaving nothing behind, and a staged archive landing under `<data>/updates`.
- The window's acceptance tests go through the accessibility tree: `--tree --offline` must name `sessions`, `transcript`, `terminal` and `files`, a tab switch must move the active session, and a question must be answerable by a digit and Enter. A pane count reads the frame's own `accessibility_json`, not `Runtime::elements`, which keeps an outgoing pane listed for its exit animation. The strip and pane are proven in `tests/wiring/tabs`: a file row opens a closable tab, a second file opens a second tab and a third press focuses the first, closing the shown tab (through `close_tab`, because the close control's press path cannot be claimed headlessly) returns to the session with the tree's selection kept, and Escape leaves a file pane or the settings for the transcript. `tests/wiring/preview` proves the preview takes the transcript's place, never shares pixels with the tree, and the header button returns the same way.
- Terminal tests replay recorded byte streams and assert grid contents, cursor position, styles and wide characters; a flood fixture asserts the frame build stays under 150 ms in release and a stated larger budget in a dev profile, with the split between Ghostty's Zig side and our frame builder reported.
- `scripts/live-probe.sh` runs the real loop against free OpenRouter models and prints a verdict; it is a manual check, not part of the gate.
- `scripts/verify-bundle.sh` proves the staged bundle: plist fields match their sources, the signature verifies, the binary is Mach-O arm64, the icon decodes.

## Shipped and not shipped

Shipped: session list and a single tab strip over JSONL logs, with one pane value (`Session`, `File`, `Settings`) and closable file tabs; a streaming markdown transcript with expandable tool cards, notes, per-turn summaries, a wheel-scrolled viewport and a single selection group; nine builtins (`read`, `write`, `edit`, `patch`, `bash`, `jobs`, `list`, `grep`, `glob`) with confinement, previews and approvals, including background jobs with per-worktree caps and logs; `skill`, `task` (one child turn, approval-free tools only, no grandchildren) and `execute` (Code Mode, 30 s / 32 calls / 64 KiB); `ask_user` with multi-pick and free text; slash commands (`/new`, `/fork`, `/rename`, `/model`, `/compact`, `/theme`, `/terminal`, `/editor`, `/sessions`, `/skills`, `/skill`, `/export`, `/help`, `/quit`, `/clean`) with a filtered menu, a ⌘K palette over the same table plus four window actions, a ⌘P file finder overlay, auto-title after the first turn, session delete and transcript export; a context meter and compaction (automatic on a too-long refusal, manual through `/compact`); a worktree tree with git state that scrolls, opens a file's preview, and hands it to Zed; Ghostty terminal with scrollback, selection copy, bracketed paste and a drawn cursor; a settings pane with a provider model picker, a session-only model change, a `0600` secrets file for the provider key and keychain-backed MCP tokens; MCP over stdio and HTTP with OAuth, namespaced tools, reconciliation and per-call notes; the wasm plugin host loaded from a directory, with tools, panels, control presses, session events and trap isolation; hot reload of settings, plugins and servers with no restart; an updater that checks a manifest, offers a release and stages a SHA-256-verified archive; the bundle scripts and the live probe.

Not shipped: a notarized or universal bundle; a DMG; an updater that installs itself; a pinned headless press path for a file tab's hover-only close chip (it is hover-only, so the test drives `close_tab`); `webfetch`/`websearch` and the board tools, whose crates are built and tested but never registered by the app; saved permission rules are not read by the app's dispatcher, and `AGENTS.md`/`CLAUDE.md` are assembled but not sent as the system prompt; git branch, ahead/behind and a diff view; per-cell accessibility (the tree names regions, text rows, and the terminal's caret and selection); drawing the MCP host's connection and call notes (only a failed server is said, in the right-hand column); editing files in the app.

## Open questions

- Whether the transcript should keep full tool output on disk for replay, and for how long.
- Which wasmtime component-model features to permit (WASI clock and randomness, nothing else) before a plugin needs more.
- Whether a live OAuth server belongs in the test suite, or whether the fake credential store is the right ceiling.
- When the install step should exist at all: a relaunch that swaps the bundle, or a person unpacking an archive after quit.
