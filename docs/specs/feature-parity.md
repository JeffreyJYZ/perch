# Feature parity: opencode vs perch

Status: inventory. Reference read out of `~/dev/monorepos/opencode` (37 protocol groups in `packages/protocol/src/groups`); target read out of `crates/*/src` and `docs/`.

Method: a line exists only where code was found. **HAVE** = a person can do it today; **PARTIAL** = some of it exists, the rest is named; **MISSING** = no code found, and it is not a judgement about difficulty.

opencode proof is the endpoint id or file that defines the capability. perch proof is the file that implements it.

## 1. Sessions

- List every session with its title, worktree and model — **HAVE** — `crates/perch-app/src/app/boot.rs` (`headers`), `crates/perch-app/src/app/sessions.rs` (`metas`) — opencode `session.list`
- Start a session on an existing worktree — **HAVE** — `crates/perch-app/src/app/sessions.rs` (`new_session`) — opencode `session.create`
- Pick a folder for a new session from a native dialog — **HAVE** — `crates/perch-ui/src/shell/folder.rs`, `crates/perch-app/src/app/sessions.rs` (`open_folder`) — opencode `prompt/directory-completion.ts`
- Switch between the sessions open in this run — **HAVE** — `crates/perch-app/src/app/sessions.rs` (`switch`) — opencode `session.active`
- Reopen a session after relaunch with its transcript and model context — **HAVE** — `crates/perch-app/src/app/boot/replay*` (the log folded through the same `AgentEvent` path a live turn uses: replies, tool cards with status and output, summaries and the `cancelled` word; the last 8 exchanges and 8 KiB of history, newest kept, with one note naming how many were left out; a torn line skipped and counted; nothing sent on open) plus `perch-store/src/session/logged.rs` — tests `crates/perch-app/tests/reopen/` (8)
- Rename a session — **HAVE** — `/rename` and `AppState::rename_session`, persisted through `SessionLog::set_title`, the sidebar row redrawn — tests `crates/perch-app/tests/sessions/commands.rs` — opencode `session.update`, `/rename`
- Delete a session — **HAVE** — the sidebar's arm-then-remove control, which also stops the bridge and drops the runtime — test `the_sidebar_delete_control_arms_then_removes_and_the_window_follows` — opencode `session.remove`
- Fork a session from an earlier message — **HAVE** — `crates/perch-store/src/session/fork.rs` (`SessionLog::fork`: new id, header rewritten, source bytes untouched, cut only after a `TurnEnd` and an interior boundary refused naming why) plus `crates/perch-app/src/app/fork.rs` and `/fork` — 6 store + 3 app tests — opencode `session.fork`, `routes/session/dialog-fork.tsx`
- Undo/redo by reverting to a boundary and restoring its files — **MISSING** — opencode `session.revert.stage`, `session.revert.commit`; `packages/core/src/session/revert.ts`
- Export or import a transcript — **MISSING** — the raw JSONL log sits at `<data dir>/<id>.jsonl` with no export path — opencode `experimental.session.export`, `experimental.session.import`
- Search sessions or inside a transcript — **MISSING** — no search over `crates/perch-store/src/session.rs`
- Generate a session title from the conversation — **HAVE** — `crates/perch-app/src/app/title.rs` (one request per session after the first turn that finished on its own, off the frame and off the turn; a cancelled or failed turn is skipped and recorded as asked; the answer is cut to one line of six words; `/rename` always wins) — tests in `crates/perch-app/tests/title/` — opencode `packages/core/src/session/title.ts`
- Show unread or seen state per session — **MISSING** — opencode `session.view`
- Move a session to another directory at a delivery boundary — **MISSING** — opencode `session.move`
- Aggregate session stats over a time range — **MISSING** — opencode `experimental.session.stats`, `feature-plugins/system/stats.tsx`
- Diff the files one turn changed — **MISSING** — opencode `session.diff`, `packages/core/src/session/diff.ts`

## 2. Turns

- Prompt a turn and stream text into the transcript — **HAVE** — `crates/perch-agent/src/turn.rs`, `crates/perch-ui/src/transcript/mod.rs` — opencode `session.prompt`
- Queue a prompt typed mid-turn and run it when the turn ends — **HAVE** — `crates/perch-app/src/session.rs` (`queue`, `start_queued`) — opencode `session.inbox.list`
- Steer a running turn with a mid-turn message — **MISSING** — queue only, never delivered into the turn in flight — opencode `session.inbox.update` (`delivery: "steer"`)
- Cancel a turn from the keyboard — **HAVE** — `crates/perch-ui/src/shell/actions.rs` (`interrupt`), `crates/perch-agent/src/cancel.rs` — opencode `session.interrupt`
- Retry a failed request with backoff, honouring `retry-after`, only before any output — **HAVE** — `crates/perch-agent/src/retry.rs`, `crates/perch-agent/src/open.rs` — opencode `packages/core/src/model-retry.ts`
- Auto-compact when the provider rejects a request as too long — **HAVE** — `crates/perch-agent/src/compact.rs` — opencode `packages/core/src/session/compaction.ts`
- Compact on demand — **HAVE** — `/compact` at `crates/perch-ui/src/shell/actions.rs:133` — opencode `session.compact`
- Cap the rounds of a turn and ask for a written report at the cap — **HAVE** — `crates/perch-agent/src/rounds.rs` (`MAX_ROUNDS`), `crates/perch-agent/src/report.rs` — opencode `session/runner/max-steps.ts`
- Show model reasoning apart from the answer — **HAVE** — `crates/perch-ui/src/transcript/view/item.rs` — opencode `routes/session/thinking-syntax.ts`
- Show token totals for a turn — **HAVE** — `crates/perch-agent/src/rounds.rs:96`, `crates/perch-ui/src/transcript/mod.rs` — opencode `session.context`
- Per-step usage rather than per-turn only — **PARTIAL** — usage is summed per round in `crates/perch-agent/src/rounds.rs` but reported once at `TurnEnd` — absent: a per-step readout
- Show what a turn cost in money — **MISSING** — no price table or currency anywhere; only tokens in `crates/perch-ui/src/shell/meter.rs` — opencode `experimental.usage.limits`
- Show how much of the context window is used — **HAVE** — `crates/perch-ui/src/shell/meter.rs` — opencode `feature-plugins/sidebar/limit.tsx`
- Switch the model for the rest of the session from the composer — **HAVE** — `crates/perch-app/src/app/models.rs`, `crates/perch-ui/src/shell/model_picker.rs` — opencode `session.switchModel`
- Run a shell command inside the session from the prompt — **PARTIAL** — only the model's `bash` tool; a person's shell is the terminal dock — opencode `session.shell`, `shell.create`
- Background a turn and list background jobs — **MISSING** — opencode `session.background`, `experimental.job.list`
- Recall earlier prompts with history and frecency — **MISSING** — no prompt history in `crates/perch-ui/src/shell/composer.rs` — opencode `prompt/history.tsx`, `prompt/frecency.tsx`
- Stash a draft prompt and come back to it — **MISSING** — opencode `prompt/stash.tsx`
- Ask the user a structured multi-field form — **PARTIAL** — `ask_user` takes options plus free text only — opencode `session.form.create`, `session.form.reply`

## 3. Tools

- Read a file, line-numbered and size-capped — **HAVE** — `crates/perch-tools/src/builtin/read.rs` — opencode `tool/plugin/read.ts`
- Write a file — **HAVE** — `crates/perch-tools/src/builtin/write.rs` — opencode `tool/plugin/write.ts`
- Replace text in a file and return a diff — **HAVE** — `crates/perch-tools/src/builtin/edit.rs` — opencode `tool/plugin/edit.ts`
- Apply a multi-file patch — **HAVE** — `crates/perch-tools/src/builtin/patch*.rs` (parse → apply → write, all-or-nothing, `--- /dev/null` creates) — opencode `tool/plugin/patch.ts`, `file-diff.ts`
- Match file paths by glob — **HAVE** — `crates/perch-tools/src/builtin/glob.rs` (`globset` + the ignore walker, refuses an escape) — opencode `tool/plugin/glob.ts`
- Search file contents with ripgrep — **HAVE** — `crates/perch-tools/src/builtin/grep.rs` — opencode `tool/plugin/grep.ts`
- List a directory — **HAVE** — `crates/perch-tools/src/builtin/list.rs` — opencode `fs.list`
- Run a shell command with a timeout and truncated output — **HAVE** — `crates/perch-tools/src/builtin/bash.rs` — opencode `tool/plugin/shell.ts`
- Confine every path to the session worktree — **HAVE** — `crates/perch-tools/src/confine.rs` — opencode `packages/core/src/file-access.ts`
- Put a question to the user and wait for the answer — **HAVE** — `crates/perch-agent/src/ask.rs` — opencode `tool/plugin/question.ts`
- Fetch a URL into the conversation — **HAVE** — `crates/perch-web/src/fetch*` (http/https only, HTML reduced to markdown, 64 KiB cap with the cut named, binary reported by type, non-2xx as a message; 30 tests against fixtures, no network) — opencode `tool/plugin/webfetch.ts`
- Search the web — **HAVE** — `crates/perch-web/src/search*` (public endpoint, title/url/snippet, deduplicated, a challenge or empty page reported with status and the first 200 characters rather than as "no results") — opencode `tool/plugin/websearch.ts`, `websearch.query`
- Keep a todo list the model edits — **HAVE** — `crates/perch-tools/src/builtin/todo*.rs` (`todowrite` replaces the list, `todoread` returns it; a process-wide table keyed by worktree root, 32-item cap, at most one `in_progress`) plus a read-only PLAN panel in `crates/perch-ui/src/shell/todo.rs` drawn from the same table — 32 tool tests + a frame test — opencode `tool/plugin/todo.ts`, `feature-plugins/sidebar/todo.tsx`
- Delegate work to a subagent — **HAVE** — `task` in `crates/perch-agent/src/subagent/*` (a child turn with its own loop and round cap, the parent's cancel token, a 64 KiB brief and result cap) wired through `crates/perch-app/src/app/subagent/*` into the registry; the child's registry is filtered to tools that need no approval and never contains `task`, so a task has no grandchildren and cannot write — tests `crates/perch-app/tests/live/subagent.rs` (2 end-to-end) — opencode `tool/plugin/subagent.ts`, `packages/core/src/session/subagent-job.ts`
- Run several tool calls in one program (Code Mode) — **HAVE** — `crates/perch-codemode` (`execute`: a program calling tools, budgets 30 s / 32 calls / 64 KiB, a tool needing approval is refused through it), registered in `crates/perch-app/src/sources.rs` — opencode `packages/codemode`
- Load a skill into the conversation — **MISSING** — opencode `tool/plugin/skill.ts`, `experimental.session.skill`
- Read an MCP server's resources as a tool result — **MISSING** — MCP tools only, no resource tool — opencode `tool/plugin/mcp-resource.ts`, `mcp.resource.catalog`
- Generate an image as a tool result — **MISSING** — opencode `tool/plugin/image-generate.ts`, `experimental.generate.image`
- Create and update a rendered artifact — **MISSING** — opencode `tool/plugin/artifact.ts`, `experimental.artifacts.*`
- Schedule a prompt to wake the session later — **MISSING** — opencode `tool/plugin/schedule.ts`, `experimental.schedule.create`
- Let the model control sessions (list, rename, move) — **MISSING** — opencode `tool/plugin/opencode.ts`
- Show one card per call, collapsed with a count of hidden lines, expandable — **HAVE** — `crates/perch-ui/src/transcript/card.rs` — opencode `util/collapse-tool-output.ts`
- Approve a call with a preview: once, for the session, or reject — **HAVE** — `crates/perch-agent/src/dispatch.rs`, `crates/perch-ui/src/shell/actions.rs` (`approve`, `deny`), `crates/perch-ui/src/shell/keymap.rs` — opencode `session.permission.reply`
- Keep an approval rule across sessions in a saved permission file — **PARTIAL** — `crates/perch-agent/src/rules/{mod,pattern}.rs` (0600 file, exact or tool-wide, `remember`/`forget`, a saved allow never widens past the worktree, corrupt is an error and never an allow, 7 tests) — absent: `Rules::load` passed to the dispatcher at boot, and a settings control to forget one — opencode `permission.saved.list`, `permission.saved.update`

## 4. Agents

- Choose between several configured agents — **MISSING** — one hard-coded system prompt, `crates/perch-app/src/app/boot.rs:116` — opencode `agent.list`, `session.switchAgent`
- Give an agent its own model and permissions — **MISSING** — opencode `agent.get`, `packages/core/src/agent.ts`
- Load machine-wide and project instructions into the system prompt — **HAVE** — `crates/perch-agent/src/instructions.rs` (`for_session`: `~/.config/perch/AGENTS.md`, `~/.config/opencode/AGENTS.md`, `~/.agents/AGENTS.md`, `~/.claude/CLAUDE.md`, then every `AGENTS.md`/`CLAUDE.md` from the worktree up to the root, outermost first so the deepest wins; 32 KiB per file, 96 KiB total, the cut named; a symlink is followed only inside the worktree; `PERCH_HOME` overrides home so a test never reads the real machine) wired at `app/boot.rs:104` — 12 tests — opencode `packages/core/src/instruction-discovery.ts`
- Activate a skill by name from the prompt — **HAVE** — `crates/perch-agent/src/skills*.rs` (`SKILL.md` front matter, machine roots plus every `.perch/skills` from the worktree up, nearest wins on a duplicate, 64 KiB body cut named) with the `skill` tool registered in `crates/perch-app/src/sources/build.rs`, `/skills` and `/skill <name>` in the composer — 9 agent tests + 5 command tests — opencode `skill.list`, `/skills`
- Extend the tool set from a plugin or an MCP server without touching the agent — **HAVE** — `crates/perch-tools/src/registry.rs`, `crates/perch-plugins/src/tool.rs`, `crates/perch-mcp/src/tools.rs` — opencode `tool/runtime.ts`

## 5. Commands

- Type a slash command with autocomplete — **HAVE** — `crates/perch-ui/src/shell/{commands*,composer_menu}.rs` (a registry of twelve commands, `/` opens a filtered list, Tab/Enter takes it, Escape closes and keeps the text) — tests `crates/perch-ui/tests/commands/` — opencode `command.list`, `component/prompt/autocomplete.tsx`
- Compact the conversation — **HAVE** — the only command, `crates/perch-ui/src/shell/actions.rs:133` — opencode `/compact`
- Define a custom command in config or markdown — **MISSING** — opencode `packages/core/src/command.ts`, `session.command`
- Open a command palette for app actions — **HAVE** — `crates/perch-ui/src/shell/{palette,palette_take,palette_view}.rs` (the same registry plus the app actions that already exist, filtered as you type) — tests `crates/perch-ui/tests/commands/palette.rs` — opencode `component/command-palette.tsx`, `command.palette.show`

## 6. Files and editor

- Browse the worktree as a tree — **HAVE** — `crates/perch-files/src/tree.rs`, `crates/perch-ui/src/files/tree.rs` — opencode `fs.list`
- Mark changed files with their git status — **HAVE** — `crates/perch-files/src/status.rs` — opencode `vcs.status`
- Show the worktree's branch, ahead and behind — **HAVE** — `crates/perch-files/src/branch.rs` (`branch_of`, one `git status --porcelain=v2 --branch`), 4 parser tests — opencode `vcs.get`
- Open a file read-only in a tab — **HAVE** — `crates/perch-files/src/preview.rs`, `crates/perch-ui/src/preview.rs` — opencode `fs.read`
- Edit a file in the app — **MISSING** — read-only by design, `crates/perch-files/src/preview.rs`, `docs/specs/2026-11-10-perch-design.md` non-goals — opencode `experimental.fs.write`
- Hand a file to Zed — **HAVE** — `crates/perch-files/src/open.rs` — opencode `$EDITOR` route
- Find a file by name across the project — **HAVE** — ⌘P finder in `crates/perch-ui/src/shell/finder*.rs` (fuzzy subsequence on the path, 12 rows with the total named, scan capped and skipping `target/`, `.git/` and gitignored paths, list read when the overlay opens) — 8 tests `crates/perch-ui/tests/finder/` — opencode `fs.find`
- Find and replace inside the open file — **MISSING** — no buffer to edit, `crates/perch-ui/src/files/editor.rs` exposes only `open_in_zed` — opencode `tool/plugin/edit.ts` in the editor
- Report a formatter or parse failure for the file being viewed — **MISSING** — no formatter or diagnostics in `crates/*/src` — opencode `packages/core/src/formatter.ts`
- View a git diff — **PARTIAL** — `crates/perch-files/src/diff.rs` (`file_diff`, `worktree_diff`, untracked as added lines, 256 KiB cap, binary named) — absent: the view that draws it — opencode `vcs.diff`, `feature-plugins/system/diff-viewer.tsx`
- Create, remove or switch a worktree — **MISSING** — a session binds to an existing one — opencode `worktree.create`, `worktree.remove`
- See the tree change when the agent writes a file — **PARTIAL** — `Workspace::refresh` on demand only, `crates/perch-files/src/tree.rs:53`; the watcher covers settings and plugins, `crates/perch-app/src/watch.rs` — absent: a worktree filesystem watch

## 7. Terminal

- Run a real PTY with colours, alternate screen and 10k rows of scrollback — **HAVE** — `crates/perch-term/src/session/mod.rs`, `crates/perch-term/src/scrollback.rs` — opencode `pty.create`, `pty.connect`
- Keep several terminal tabs, one per shell — **HAVE** — `crates/perch-ui/src/shell/dock.rs`, `crates/perch-ui/src/shell/actions.rs` (`open_terminal`) — opencode `component/prompt/terminals-tab.tsx`
- Select and copy terminal text — **HAVE** — `crates/perch-term/src/selection.rs` — opencode `component/session-frame.tsx` selection
- Paste with bracketed-paste markers — **HAVE** — `crates/perch-term/src/paste.rs` — opencode `pty.connect`
- Resize the grid with the window — **HAVE** — `crates/perch-term/src/session/handle.rs` (`resize`), `crates/perch-ui/src/shell/state.rs` (`grid`) — opencode `pty.update`
- Keep each session's terminals with that session — **HAVE** — `crates/perch-ui/src/shell/state.rs:39` — opencode `pty.list`
- Split the terminal or the editor pane — **MISSING** — one dock, no split in `crates/perch-ui/src/shell/layout.rs` — opencode `component/session-frame.tsx` (`/terminal` beside the session)
- Choose the shell command in settings — **MISSING** — the shell is `$SHELL`, no field in `crates/perch-store/src/settings.rs` — opencode `config.shells`
- Survive a restart with the terminal still attached — **MISSING** — opencode `experimental.persistentPty.*`

## 8. MCP and plugins

- Connect a local stdio MCP server — **HAVE** — `crates/perch-mcp/src/stdio.rs`, `crates/perch-mcp/src/host.rs` — opencode `mcp.list`
- Connect a remote HTTP MCP server — **HAVE** — `crates/perch-mcp/src/remote.rs` — opencode `mcp/index.ts`
- Complete an MCP OAuth flow and keep the token — **HAVE** — `crates/perch-mcp/src/oauth/flow.rs`, `crates/perch-mcp/src/oauth/store.rs` — opencode `mcp/oauth.ts`
- Namespace MCP tool names per server and reconcile on config change — **HAVE** — `crates/perch-mcp/src/host.rs` (`reconcile`), `crates/perch-mcp/src/lib.rs` (`separate`) — opencode `mcp.pool.ts`
- Add, connect or disconnect a server while the app runs — **PARTIAL** — by editing `settings.toml` and letting the watcher reload; no in-app control — absent: add/connect UI — opencode `experimental.mcp.add`, `experimental.mcp.connect`
- Show MCP connection state and per-call notes — **PARTIAL** — notes are recorded in `crates/perch-mcp/src/host.rs` (`notes`) but only a failed server is drawn — opencode `feature-plugins/sidebar/mcp.tsx`
- List and read MCP resources — **MISSING** — opencode `mcp.resource.catalog`
- Add a tool from a wasm plugin — **HAVE** — `crates/perch-plugins/src/tool.rs`, `crates/perch-plugins/src/load.rs` — opencode `plugin.list`
- Draw a plugin side panel with controls — **HAVE** — `crates/perch-plugins/src/panel.rs`, `crates/perch-ui/src/plugin_panel.rs` — opencode `plugin.gui`
- Feed session events to a plugin — **HAVE** — `crates/perch-plugins/src/feed.rs` — opencode `event.subscribe`
- Isolate a trapping plugin from the frame — **HAVE** — `crates/perch-plugins/src/bounds.rs` — opencode `plugin/effect`
- Install or update a plugin from a registry — **MISSING** — plugins are read from a directory — opencode `plugin.check`, `plugin.update`
- Run npm/TypeScript plugins — **MISSING** — wasm only, `crates/perch-plugins/src/abi.rs` — opencode `plugin.list` (npm)

## 9. Providers, credentials and models

- Point the app at one OpenAI-compatible endpoint with one key — **HAVE** — `crates/perch-agent/src/provider/endpoint.rs`, `crates/perch-app/src/settings.rs` — opencode `provider.get`
- List models from the provider and pick one by search — **HAVE** — `crates/perch-agent/src/provider/models.rs`, `crates/perch-ui/src/shell/model_picker.rs` — opencode `model.list`
- Override the model for one session — **HAVE** — `crates/perch-app/src/app/models.rs` — opencode `session.switchModel`
- Keep several providers and credentials and activate one — **MISSING** — one endpoint and one key — opencode `credential.create`, `credential.activate`
- Log in to a provider with OAuth or a device code — **MISSING** — opencode `integration.oauth.connect`, `integration.command.connect`
- Choose a reasoning effort or variant per model — **MISSING** — no effort field in `crates/perch-store/src/settings.rs` — opencode `variant.ts`, `/variants`
- Store the provider key in a 0600 file — **HAVE** — `crates/perch-app/src/secrets.rs`, `crates/perch-store/src/secret.rs` — opencode `credential.*`
- Read the provider's remaining usage limits — **MISSING** — opencode `experimental.usage.limits`, `/usage`
- Attach an image to a prompt — **MISSING** — text-only wire, `crates/perch-agent/src/provider/wire.rs` — opencode `experimental.media.get`, `MediaInfo`

## 10. Settings, themes and keybinds

- Change base URL, model, theme and key in a settings pane — **HAVE** — `crates/perch-ui/src/shell/settings*.rs`, `crates/perch-app/src/settings.rs` — opencode `opencode.settings`, `config.get`
- Pick a dark or light theme — **HAVE** — two themes, `crates/perch-ui/src/settings.rs:17` — opencode `theme/assets/*` (34)
- Pick from a theme library — **MISSING** — two names only — opencode `theme/discovery.ts`
- Rebind a key — **MISSING** — fixed chords in `crates/perch-ui/src/shell/keymap.rs` — opencode `config/keybind.ts`
- Set font family or size — **MISSING** — no such field in `crates/perch-store/src/settings.rs` — opencode `/themes`, `settings`
- Edit MCP servers without leaving the app — **MISSING** — TOML only, `crates/perch-store/src/settings.rs` — opencode `experimental.mcp.add`
- Reload settings without a restart — **HAVE** — `crates/perch-app/src/watch.rs`, `crates/perch-app/src/settings.rs` — opencode `location.reload`
- Per-project configuration — **MISSING** — one global file — opencode `project.update`, `config.get`

## 11. Everything else a person can feel

- Check for an update and stage a verified download — **HAVE** — `crates/perch-update/src/check.rs`, `crates/perch-app/src/update/worker.rs` — opencode `/update`
- Install the update in place — **MISSING** — staging only, `crates/perch-app/src/update/status.rs` — opencode `service.restart`
- Reopen a closed turn's full tool output from disk — **MISSING** — `docs/specs/2026-11-10-perch-design.md` open questions — opencode `session.message.get`
- Search the web from the app — **MISSING** — opencode `websearch.query`, `/websearch`
- Talk to the model by voice — **MISSING** — opencode `experimental.voice.session`
- Run a prompt on a schedule or cron — **MISSING** — opencode `experimental.schedule.create`
- List background jobs (shell, subagent, image) — **HAVE** — `crates/perch-tools/src/builtin/{jobs,job}.rs` (`jobs` list/read/kill by process group, process-wide table per worktree, 16-job cap, no child outlives the app) plus `bash`'s `background: true` — opencode `experimental.job.list`
- Coordinate the sessions in one project over a shared board — **HAVE** — `crates/perch-board` (`board`: read what this session has not seen, or post; `board_wait`: match with a 30 s cap; one append-only JSONL per project, a cursor per session, a malformed line skipped) — `docs/specs/orchestrator.md` — opencode has no equivalent
- Pair a phone or web client to the same server — **MISSING** — single-window process — opencode `server.pair`, `server.connect`
- Serve several projects from one process — **MISSING** — one process, one app — opencode `location.get`, `project.list`
- Stream events to an external client — **MISSING** — channels only, no socket — opencode `event.subscribe`
- Expose an accessibility tree — **PARTIAL** — named regions and rows plus `--tree`, `crates/perch-app/src/main.rs` — absent: per-cell terminal accessibility — opencode is a terminal UI
- Lose at most the line being written on a crash — **HAVE** — append-only log, `crates/perch-store/src/session.rs` — opencode `packages/core/src/session/store.ts`
- Import a v1 session history — **MISSING** — opencode `experimental.migration.v1.status`

## The 12 highest-value gaps, in the order a person meets them

1. **Resume a session with its transcript and context** — `perch-app`, `perch-store` — `app/boot.rs`, `session.rs`, `store/session.rs`, `agent/turn.rs` — cheap: the log and the parser exist, so it is a replay into `Transcript` plus a seed message list; the only real design choice is what a resumed session re-sends to the model.
2. **Rename and delete a session** — `perch-store`, `perch-app`, `perch-ui` — `store/session.rs` (meta rewrite), `app/sessions.rs`, `shell/sidebar.rs` — cheap: a title field and one row control; delete must also stop the bridge and drop the runtime.
3. **Slash commands with autocomplete** — `perch-ui`, `perch-app` — `shell/composer.rs`, `shell/actions.rs`, new command registry — moderate: the composer already intercepts `/compact`; the work is a registry, a filter list and keys that do not collide with the answer keys.
4. **Project instructions (AGENTS.md) in the system prompt** — `perch-app`, `perch-agent` — `app/boot.rs` (`SYSTEM`), new instruction loader — cheap: read files up the worktree, concatenate, prepend; the risk is only context cost.
5. **Undo a turn and restore the files it changed** — `perch-agent`, `perch-app`, `perch-files` — new checkpoint module, `agent/turn.rs`, `app/session.rs` — expensive: needs a snapshot per step and a restore path that does not fight the agent's own writes.
6. **Search sessions and transcript text** — `perch-app`, `perch-ui` — `store/session.rs`, `shell/sidebar.rs`, new search view — cheap for the open transcript, expensive for history because the JSONL logs need an index.
7. **A todo tool with a sidebar panel** — `perch-agent`, `perch-ui` — `agent/tools.rs`, new `builtin/todo.rs`, `shell/sidebar.rs` — cheap: a registered tool plus one read-only panel; no new protocol.
8. **Saved permission rules** — `perch-agent`, `perch-store` — `agent/dispatch.rs`, `store/settings.rs` — cheap: the dispatcher already has `once`/`always`/`reject`; a rule is that decision written to a file and read at start.
9. **Money cost per turn** — `perch-agent`, `perch-ui` — `agent/rounds.rs`, `shell/meter.rs`, new price table — moderate: the arithmetic is trivial, keeping a model price table current is not.
10. **Glob and patch tools** — `perch-tools` — `builtin/mod.rs`, new `builtin/glob.rs` and `builtin/patch.rs`, `registry.rs` — cheap: the registry is name-dispatched and both tools reuse `confine` and the existing read/write bodies.
11. **Git diff view and branch/ahead-behind** — `perch-files`, `perch-ui`, `perch-app` — `files/status.rs` (already shells git), new diff view, `app/sessions.rs` (`branch`), `shell/sidebar.rs` — moderate: the git plumbing exists; the cost is a diff renderer and a patch parser.
12. **Subagents and skills** — `perch-agent`, `perch-app`, `perch-ui` — `agent/turn.rs`, new `agent/subagent.rs`, `agent/tools.rs`, `app/sessions.rs` (nested sessions), `shell/transcript` — expensive: a second agent loop with its own session, cancellation, cost accounting and UI, and the session store assumes one log per window.
