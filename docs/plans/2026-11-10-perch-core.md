# perch core harness — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A single Rust binary that opens a Rune window, runs a session against an OpenAI-compatible provider, streams a transcript, executes six tools behind approvals, and offers a worktree file browser and a Ghostty-backed terminal.

**Architecture:** One process. The UI draws on the main thread from live values; the agent, tools and terminal each own a thread and report typed events over channels. Terminal emulation is `libghostty-vt`, which is `!Send`, so one thread owns the `Terminal` and the UI receives plain data snapshots.

**Tech Stack:** Rust (edition 2024), `rune` (pinned rev, UI), `libghostty-vt` 0.2.2 (terminal core), `portable-pty` (spawning), `reqwest` (streaming HTTP), `serde`/`serde_json`, `gix` (git), `ignore` (walk), `keyring` (secrets).

**Spec:** `docs/specs/2026-11-10-perch-design.md`

## Global Constraints

- macOS only; no Windows or Linux code paths in v1.
- No Node, no JavaScript, no webview anywhere.
- Rune is pinned to one `dev` revision in the workspace manifest; the pin is bumped deliberately, never by a floating branch.
- `libghostty-vt` and `libghostty-vt-sys` are pinned to the same exact version.
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` must pass before every commit.
- No blocking call without a timeout: every process wait, HTTP request and channel receive that can hang carries one.
- A path outside the session's worktree is refused, never clipped; symlinks are resolved before the check.
- Provider keys live in the keychain; never in TOML, never in a session file, never in a log line.
- Files are read-only in the app. Nothing but the agent writes into a worktree.
- Every capability a plugin will need is designed as a registry from the start: tools are registered dynamically, panels are declarative, and no built-in gets a private path the plugin host cannot take.

## Review Focus

- Modifier and function keys produce the bytes a real shell expects, including Ctrl, Alt, Fn-style arrows and bracketed paste.
- Resize during a full-screen program reflows the grid without losing the cursor or the alternate screen.
- A provider stream cut mid tool-call never yields a half-parsed argument set, and the turn still ends in a definite state.
- Cancelling while a tool runs leaves no orphan process and no half-applied edit.
- `../` traversal and symlinks pointing outside the worktree are refused by every read path, including the preview and the git diff.

---

### Task 1: Workspace, settings and session log

**Files:**
- Create: `Cargo.toml` (workspace), `crates/perch-store/Cargo.toml`, `crates/perch-store/src/lib.rs`, `crates/perch-store/src/settings.rs`, `crates/perch-store/src/secret.rs`, `crates/perch-store/src/session.rs`
- Test: `crates/perch-store/tests/session.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Settings { provider_base_url: String, model: String, theme: String }`; `Settings::load(dir: &Path) -> Result<Settings>`; `secret::store(account: &str, key: &str) -> Result<()>`; `secret::load(account: &str) -> Result<Option<String>>` (keyring service `dev.jyz.perch`, never written to TOML); `SessionMeta { id: String, title: String, worktree: PathBuf, branch: String, model: String, created: u64 }`; `SessionLog::open(dir: &Path, meta: SessionMeta) -> Result<SessionLog>`; `SessionLog::append(&mut self, event: &SessionEvent) -> Result<()>`; `SessionLog::events(&self) -> Result<Vec<SessionEvent>>`; `SessionEvent` with serde-tagged variants `User { text }`, `Assistant { text }`, `ToolCall { id, name, args }`, `ToolResult { id, ok, text }`, `TurnEnd { reason, tokens_in, tokens_out }`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn append_then_read_returns_events_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let meta = meta_for(dir.path(), "perch-dev");
    let mut log = SessionLog::open(dir.path(), meta.clone()).unwrap();
    log.append(&SessionEvent::User { text: "hi".into() }).unwrap();
    log.append(&SessionEvent::Assistant { text: "hello".into() }).unwrap();
    assert_eq!(SessionLog::open(dir.path(), meta).unwrap().events().unwrap().len(), 2);
}

#[test]
fn a_truncated_last_line_is_dropped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let meta = meta_for(dir.path(), "perch-dev");
    let mut log = SessionLog::open(dir.path(), meta.clone()).unwrap();
    log.append(&SessionEvent::User { text: "hi".into() }).unwrap();
    drop(log);
    let path = meta_path(dir.path(), &meta.id);
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"kind\":\"assis").unwrap();
    assert_eq!(SessionLog::open(dir.path(), meta).unwrap().events().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-store`
Expected: FAIL, `SessionLog` not found.

- [ ] **Step 3: Implement**

`SessionEvent` is `#[serde(tag = "kind", rename_all = "snake_case")]`. `SessionLog::open` writes the `SessionMeta` as line 1 when the file is empty, so a session's worktree, branch and model travel with its log; `events()` skips that line when it parses as metadata, which a `SessionEvent` can never do. Any line that fails to parse is dropped and the rest still load — that is the real rule, not "the last line", because a complete line without a trailing newline is valid. `Settings::save` exists and `load` writes defaults when the file is absent; the file is `settings.toml` and its path is exposed by `settings::path_in`. `meta_path(dir, id)` is `dir.join(format!("{id}.jsonl"))` and is public, so tests and later crates never re-spell it.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-store`
Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(store): settings and append-only session log"
```

---

### Task 2: Rune shell with tabs and session list

**Files:**
- Create: `crates/perch-ui/Cargo.toml`, `crates/perch-ui/src/lib.rs`, `crates/perch-ui/src/shell.rs`, `crates/perch-app/Cargo.toml`, `crates/perch-app/src/main.rs`
- Test: `crates/perch-app/tests/tree.rs`

**Interfaces:**
- Consumes: `SessionMeta` from Task 1.
- Produces: `Shell::new(sessions: Vec<SessionMeta>, active: usize) -> Shell`; `Shell::active(&self) -> usize`; `Shell::select(&mut self, index: usize)`; `Shell::tab_titles(&self) -> Vec<String>`; executable `perch`, which opens one window and dumps its accessibility tree when run with `--tree`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn selecting_a_tab_changes_the_active_session() {
    let mut shell = Shell::new(vec![meta("a"), meta("b")], 0);
    assert_eq!(shell.active(), 0);
    shell.select(1);
    assert_eq!(shell.active(), 1);
    assert_eq!(shell.tab_titles(), vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn tree_dump_names_every_region() {
    let out = Command::new(env!("CARGO_BIN_EXE_perch")).args(["--tree", "--offline"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for region in ["sessions", "transcript", "terminal", "files"] {
        assert!(text.contains(region), "accessibility tree is missing {region}:\n{text}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-app`
Expected: FAIL, `Shell` not found and the binary does not exist.

- [ ] **Step 3: Implement**

`Shell` is a plain struct; the view is three columns — session list and file tree left, transcript centre, terminal dock at the bottom — each with a stable accessibility id so the tree dump names it. `--offline` skips provider construction. `--tree` prints the rune accessibility dump and exits.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-app`
Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(ui): three-column shell with session tabs"
```

---

### Task 3: Terminal emulation on its own thread

**Files:**
- Create: `crates/perch-term/Cargo.toml`, `crates/perch-term/src/lib.rs`, `crates/perch-term/src/session.rs`, `crates/perch-term/src/frame.rs`
- Test: `crates/perch-term/tests/replay.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `TermSession::spawn(cwd: &Path, cols: u16, rows: u16) -> Result<(TermHandle, mpsc::Receiver<Frame>)>`; `TermHandle::write(&self, bytes: &[u8])`; `TermHandle::resize(&self, cols: u16, rows: u16)`; `TermHandle::key(&self, event: KeyEvent) -> Vec<u8>`; `Frame { rows: Vec<RowView>, cursor: CursorView, cols: u16, rows: u16 }`; `RowView { cells: Vec<CellView>, wrapped: bool }`; `CellView { text: String, fg: Color, bg: Color, bold: bool, italic: bool, underline: bool }`; `Color::{Default, Indexed(u8), Rgb(u8, u8, u8)}`; `replay(bytes: &[u8], cols: u16, rows: u16) -> Frame` as a test-facing free function over the same code path.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn sgr_colours_and_cursor_land_in_the_frame() {
    let frame = replay(b"hi \x1b[31mred\x1b[0m\r\nsecond", 20, 4);
    assert_eq!(frame.rows[0].cells.iter().map(|c| c.text.as_str()).collect::<String>().trim_end(), "hi red");
    assert_eq!(frame.rows[0].cells[3].fg, Color::Indexed(1));
    assert_eq!(frame.cursor.row, 1);
    assert_eq!(frame.cursor.col, 6);
}

#[test]
fn wide_character_takes_two_columns() {
    let frame = replay("日本".as_bytes(), 10, 2);
    assert_eq!(frame.rows[0].cells[0].text, "日");
    assert_eq!(frame.rows[0].cells[1].text, "");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-term`
Expected: FAIL, `replay` not found.

- [ ] **Step 3: Implement**

`Terminal` is created, driven and read on one thread; the crate is `!Send`, so the thread owns it and only `Frame` crosses the channel. The thread selects on two inputs: PTY bytes (read into a buffer) and commands (`Write`, `Resize`, `Key`, `Shutdown`). After each batch it updates a `RenderState` and builds a `Frame` from its rows and cells, using libghostty's grapheme and style accessors; wide characters leave an empty cell in the second column. `TermHandle` holds the sender plus an atomic "dirty" wake the UI polls.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-term`
Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(term): ghostty-backed grid on a dedicated thread"
```

---

### Task 4: Terminal painting, keys and resize

**Files:**
- Create: `crates/perch-ui/src/terminal_view.rs`, `crates/perch-term/src/keys.rs`
- Modify: `crates/perch-ui/src/shell.rs`
- Test: `crates/perch-app/tests/terminal.rs`

**Interfaces:**
- Consumes: `Frame`, `TermHandle` from Task 3.
- Produces: `paint_rows(frame: &Frame) -> Vec<RowRun>` (run-length grouped cells sharing a style, so a frame turns into few text elements); `encode_key(event: KeyEvent, mods: Mods) -> Vec<u8>`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn identical_styles_collapse_into_one_run() {
    let frame = replay(b"aaa\x1b[1mbbb", 10, 1);
    let runs = paint_rows(&frame);
    assert_eq!(runs[0].text, "aaa");
    assert!(runs.iter().any(|r| r.text == "bbb" && r.bold));
}

#[test]
fn control_and_arrow_keys_are_encoded() {
    assert_eq!(encode_key(Key::Char('c'), Mods::CTRL), b"\x03".to_vec());
    assert_eq!(encode_key(Key::Up, Mods::NONE), b"\x1b[A".to_vec());
    assert_eq!(encode_key(Key::Enter, Mods::NONE), b"\r".to_vec());
}

#[test]
fn flood_keeps_frame_build_under_budget() {
    let bytes = std::fs::read("tests/fixtures/flood.bin").unwrap();
    let start = std::time::Instant::now();
    let _ = replay(&bytes, 120, 40);
    assert!(start.elapsed() < std::time::Duration::from_millis(150));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-app terminal`
Expected: FAIL, `paint_rows` and `encode_key` not found.

- [ ] **Step 3: Implement**

`paint_rows` walks cells, merges neighbours with equal style, and skips the trailing empty cells of each row. `encode_key` delegates to libghostty's key encoder with an explicit modifier set, falling back to the literal byte for plain characters. Resize sends the new size to the PTY then to the grid, and the next frame is built after both. `tests/fixtures/flood.bin` is generated once by a commit-time helper (`cargo run -p perch-term --example make_flood`) and checked in.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-app terminal`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(term): paint rows, encode keys, survive resize"
```

---

### Task 5: Provider client with a recorded fixture

**Files:**
- Create: `crates/perch-agent/Cargo.toml`, `crates/perch-agent/src/lib.rs`, `crates/perch-agent/src/provider.rs`, `crates/perch-agent/src/chunk.rs`
- Test: `crates/perch-agent/tests/provider.rs`, `crates/perch-agent/tests/fixtures/sse_simple.txt`

**Interfaces:**
- Consumes: settings values from Task 1.
- Produces: `ChatRequest { base_url: String, api_key: SecretString, model: String, messages: Vec<Message>, tools: Vec<ToolSpec> }`; `stream(req: &ChatRequest) -> Result<ChunkStream>`; `ChunkStream: Iterator<Item = Result<Chunk>>`; `Chunk::Text(String)`, `Chunk::ToolCall { index: usize, id: Option<String>, name: Option<String>, args_fragment: String }`, `Chunk::Usage { input: u32, output: u32 }`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn parses_text_tool_calls_and_usage_from_recorded_sse() {
    let server = fixture_server("tests/fixtures/sse_simple.txt");
    let mut chunks = stream(&request_against(&server)).unwrap();
    let mut text = String::new();
    let mut args = String::new();
    let mut usage = None;
    while let Some(chunk) = chunks.next() {
        match chunk.unwrap() {
            Chunk::Text(t) => text.push_str(&t),
            Chunk::ToolCall { args_fragment, .. } => args.push_str(&args_fragment),
            Chunk::Usage { input, output } => usage = Some((input, output)),
        }
    }
    assert_eq!(text, "done");
    assert_eq!(args, "{\"path\":\"src/main.rs\"}");
    assert_eq!(usage, Some((12, 3)));
}

#[test]
fn a_cut_stream_ends_with_an_error_not_a_partial_tool_call() {
    let server = fixture_server("tests/fixtures/sse_cut.txt");
    let mut chunks = stream(&request_against(&server)).unwrap();
    let last = chunks.by_ref().last().unwrap();
    assert!(last.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-agent provider`
Expected: FAIL, `stream` not found.

- [ ] **Step 3: Implement**

Request body is the OpenAI chat-completions shape with `stream: true` and `stream_options: {"include_usage": true}`. The reader splits on blank lines, ignores comments, and decodes `data: [DONE]` as clean end-of-stream. Delta fragments are passed through untouched — assembling a tool call is Task 6's job — and a stream that ends without `[DONE]` yields one `Err`. `fixture_server` binds `127.0.0.1:0`, serves the file once with `content-type: text/event-stream`, and returns its base URL.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-agent provider`
Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(agent): openai-compatible streaming client"
```

---

### Task 6: Turn loop, cancellation and crystal-clear endings

**Files:**
- Create: `crates/perch-agent/src/turn.rs`, `crates/perch-agent/src/event.rs`
- Modify: `crates/perch-agent/src/lib.rs`
- Test: `crates/perch-agent/tests/turn.rs`

**Interfaces:**
- Consumes: `stream`, `Chunk` from Task 5.
- Produces: `Agent::spawn(config: AgentConfig) -> (AgentHandle, mpsc::Receiver<AgentEvent>)`; `AgentHandle::prompt(&self, text: String)`; `AgentHandle::cancel(&self)`; `AgentEvent::{TextDelta(String), ToolCall(ToolCall), ToolResult { id, ok, text }, TurnEnd { reason: EndReason, usage: Usage }}`; `EndReason::{Done, Cancelled, Errored(String)}`; `ToolCall { id: String, name: String, args: serde_json::Value }`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn tool_call_fragments_assemble_into_valid_json_or_end_errored() {
    let (handle, events) = agent_against("tests/fixtures/sse_simple.txt");
    handle.prompt("go".into());
    let seen = collect_until_end(events);
    let call = seen.tool_calls().first().unwrap();
    assert_eq!(call.name, "read");
    assert_eq!(call.args["path"], "src/main.rs");
    assert_eq!(seen.end().reason, EndReason::Done);
}

#[test]
fn cancel_yields_exactly_one_cancelled_end() {
    let (handle, events) = agent_against("tests/fixtures/sse_slow.txt");
    handle.prompt("go".into());
    handle.cancel();
    let seen = collect_until_end(events);
    assert_eq!(seen.ends().len(), 1);
    assert_eq!(seen.end().reason, EndReason::Cancelled);
}

#[test]
fn a_cut_stream_ends_errored_with_the_reason() {
    let (handle, events) = agent_against("tests/fixtures/sse_cut.txt");
    handle.prompt("go".into());
    let seen = collect_until_end(events);
    assert!(matches!(seen.end().reason, EndReason::Errored(_)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-agent turn`
Expected: FAIL, `Agent::spawn` not found.

- [ ] **Step 3: Implement**

The turn thread owns message history and a cancellation flag read between chunks. Text deltas forward immediately; tool-call fragments accumulate per `index` into `id`/`name`/`args`, and the assembled string is parsed once at the end of the stream — a parse failure becomes `EndReason::Errored`. `collect_until_end` stops at the first `TurnEnd`, and the tests assert exactly one is ever emitted.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-agent`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(agent): turn loop with cancellation and definite endings"
```

---

### Task 7: Tools, approvals and path confinement

**Files:**
- Create: `crates/perch-tools/Cargo.toml`, `crates/perch-tools/src/lib.rs`, `crates/perch-tools/src/registry.rs`, `crates/perch-tools/src/builtin.rs`, `crates/perch-tools/src/confine.rs`
- Test: `crates/perch-tools/tests/tools.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks except the worktree path.
- Produces: `trait Tool { fn spec(&self) -> ToolSpec; fn needs_approval(&self) -> bool; fn call(&self, args: &serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput>; }`; `ToolCtx { root: PathBuf }`; `ToolOutput { text: String, preview: Option<Preview> }`; `Preview::{Diff(String), Command(String)}`; `ToolRegistry::builtin() -> ToolRegistry`; `ToolRegistry::register(&mut self, tool: Arc<dyn Tool>)`; `ToolRegistry::specs(&self) -> Vec<ToolSpec>`; `ToolRegistry::call(&self, name: &str, args: &serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput>`; `confine(root: &Path, candidate: &Path) -> Result<PathBuf>`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn every_builtin_tool_is_registered_with_a_schema() {
    let names: Vec<String> = ToolRegistry::builtin().specs().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["read", "write", "edit", "bash", "list", "grep"]);
}

#[test]
fn a_registered_tool_appears_in_the_specs_and_is_callable() {
    let mut registry = ToolRegistry::builtin();
    registry.register(Arc::new(Echo));
    assert!(registry.specs().iter().any(|s| s.name == "echo"));
    let ctx = ToolCtx { root: std::env::temp_dir() };
    assert_eq!(registry.call("echo", &json!({"text": "hi"}), &ctx).unwrap().text, "hi");
}

#[test]
fn parent_traversal_and_symlink_escape_are_refused() {
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("ok.txt"), "x").unwrap();
    assert!(confine(root.path(), Path::new("../etc/passwd")).is_err());
    std::os::unix::fs::symlink("/etc", root.path().join("out")).unwrap();
    assert!(confine(root.path(), Path::new("out/hosts")).is_err());
}

#[test]
fn edit_returns_a_diff_preview_and_write_refuses_outside_root() {
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();
    let ctx = ToolCtx { root: root.path().to_path_buf() };
    let out = ToolRegistry::builtin().call("edit", &json!({"path": "a.rs", "old": "a", "new": "b"}), &ctx).unwrap();
    assert!(matches!(out.preview, Some(Preview::Diff(_))));
    assert!(ToolRegistry::builtin().call("write", &json!({"path": "../x", "text": "y"}), &ctx).is_err());
}

#[test]
fn bash_output_is_truncated_to_the_stated_budget() {
    let root = tempdir().unwrap();
    let ctx = ToolCtx { root: root.path().to_path_buf() };
    let out = ToolRegistry::builtin().call("bash", &json!({"command": "yes x | head -c 200000"}), &ctx).unwrap();
    assert!(out.text.len() <= 16 * 1024);
    assert!(out.text.contains("[truncated"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-tools`
Expected: FAIL, `Registry` not found.

- [ ] **Step 3: Implement**

`confine` canonicalises the root, joins and canonicalises the candidate, and rejects anything not prefixed by the root — which is what catches symlinks. `bash` runs with the root as cwd, a 30s timeout, both streams captured, output truncated at 16 KiB with a `[truncated N bytes]` marker. `read` caps at 64 KiB and numbers lines. `edit` requires `old` to appear exactly once and returns a unified diff. `grep` launches `rg` itself — `--line-number --no-heading --color=never --json` is unnecessary, plain text with `-n` is enough — under the same 30s timeout, so patterns are real regexes, `.gitignore` is respected and binaries are skipped by the tool that already knows how; a missing `rg` is an error naming it, never a silent empty result.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-tools`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(tools): six builtins with confinement and approvals"
```

---

### Task 8: Transcript view

**Files:**
- Create: `crates/perch-ui/src/transcript.rs`
- Modify: `crates/perch-ui/src/shell.rs`
- Test: `crates/perch-ui/tests/transcript.rs`

**Interfaces:**
- Consumes: `AgentEvent` from Task 6, `Preview` from Task 7.
- Produces: `Transcript::default()`; `Transcript::apply(&mut self, event: &AgentEvent)`; `Transcript::items(&self) -> &[Item]`; `Item::{User { text }, Assistant { text }, Tool { id, name, status, output, preview }}`; `ToolStatus::{Running, Ok, Failed, Cancelled}`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn deltas_append_to_the_open_assistant_item() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::TextDelta("Hel".into()));
    t.apply(&AgentEvent::TextDelta("lo".into()));
    assert!(matches!(&t.items()[0], Item::Assistant { text } if text == "Hello"));
}

#[test]
fn a_tool_result_updates_the_matching_card_only() {
    let mut t = Transcript::default();
    t.apply(&AgentEvent::ToolCall(call("a", "read")));
    t.apply(&AgentEvent::ToolCall(call("b", "grep")));
    t.apply(&AgentEvent::ToolResult { id: "b".into(), ok: false, text: "no match".into() });
    let statuses: Vec<_> = t.items().iter().filter_map(|i| match i {
        Item::Tool { id, status, .. } => Some((id.as_str(), *status)),
        _ => None,
    }).collect();
    assert_eq!(statuses, [("a", ToolStatus::Running), ("b", ToolStatus::Failed)]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-ui transcript`
Expected: FAIL, `Transcript` not found.

- [ ] **Step 3: Implement**

Assistant text grows one string per open turn; markdown highlights are re-derived for the tail only on each delta, not the whole document. Tool cards key on call id and show status plus a collapsed output of the first five lines. Selection is grouped across the whole transcript column so copy spans turns.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-ui transcript`
Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(ui): transcript with streaming text and tool cards"
```

---

### Task 9: Files, git and open in Zed

**Files:**
- Create: `crates/perch-files/Cargo.toml`, `crates/perch-files/src/lib.rs`, `crates/perch-files/src/tree.rs`, `crates/perch-files/src/status.rs`, `crates/perch-files/src/open.rs`
- Test: `crates/perch-files/tests/files.rs`

**Behaviour added after the first pass:** a file tracked by git and deleted from disk appears in the tree greyed out (status `Deleted`), so the panel never omits a change git knows about; clicking it reports the deletion instead of previewing nothing.

**Interfaces:**
- Consumes: nothing.
- Produces: `Workspace::open(root: &Path) -> Result<Workspace>`; `Workspace::tree(&self) -> Vec<Node>`; `Node { path: PathBuf, name: String, kind: NodeKind, status: Option<FileStatus>, children: Vec<Node> }`; `NodeKind::{Dir, File}`; `FileStatus::{Added, Modified, Deleted, Untracked, Conflicted}`; `Workspace::preview(&self, path: &Path) -> Result<PreviewText>`; `PreviewText { lines: Vec<String>, language: Option<String> }`; `open_in_zed(path: &Path) -> Result<()>`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn tree_hides_ignored_files_and_marks_git_state() {
    let repo = temp_repo();
    std::fs::write(repo.path().join("kept.rs"), "fn main() {}").unwrap();
    std::fs::write(repo.path().join("ignored.log"), "x").unwrap();
    std::fs::write(repo.path().join(".gitignore"), "*.log\n").unwrap();
    let ws = Workspace::open(repo.path()).unwrap();
    let names = flatten(&ws.tree());
    assert!(names.contains(&"kept.rs".to_string()));
    assert!(!names.contains(&"ignored.log".to_string()));
    let status = ws.tree().iter().flat_map(walk).find(|n| n.name == "kept.rs").unwrap().status;
    assert_eq!(status, Some(FileStatus::Untracked));
}

#[test]
fn preview_refuses_a_path_outside_the_root() {
    let repo = temp_repo();
    let ws = Workspace::open(repo.path()).unwrap();
    assert!(ws.preview(Path::new("../../etc/passwd")).is_err());
}

#[test]
fn language_is_inferred_from_the_extension() {
    assert_eq!(language_for(Path::new("a.rs")), Some("rust"));
    assert_eq!(language_for(Path::new("CMakeLists.txt")), Some("cmake"));
    assert_eq!(language_for(Path::new("README")), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-files`
Expected: FAIL, `Workspace` not found.

- [ ] **Step 3: Implement**

The walker is `ignore::WalkBuilder` with standard filters, sorted directories-first then alphabetically. Status comes from `git status --porcelain=v1 -z --untracked-files=all` under a 30s timeout, mapped into `FileStatus` — porcelain output is the contract the spec states, so parsing it is the direct proof. After the walk, every path git reports as deleted gets a synthetic node under its parent, with any missing parent directories created along that path only, marked `FileStatus::Deleted`; `preview` on a deleted path returns an error naming the deletion rather than an empty file. Icons map by extension through one table, returning a Material Symbols glyph name. `open_in_zed` runs `open -a Zed <path>` with an argument, never a shell string.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-files`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(files): worktree tree, git status, preview, open in zed"
```

---

### Task 10: Session tabs wired end to end

**Files:**
- Create: `crates/perch-app/src/app.rs`
- Modify: `crates/perch-app/src/main.rs`, `crates/perch-ui/src/shell.rs`
- Test: `crates/perch-app/tests/sessions.rs`

**Interfaces:**
- Consumes: `SessionLog` (Task 1), `Shell` (Task 2), `TermHandle` (Task 3), `AgentHandle` (Task 6), `Transcript` (Task 8), `Workspace` (Task 9).
- Produces: `AppState::open(sessions_dir: &Path) -> Result<AppState>`; `AppState::new_session(&mut self, worktree: &Path) -> Result<usize>`; `AppState::switch(&mut self, index: usize) -> Result<()>`; `AppState::active_session(&self) -> Option<&SessionRuntime>`; `SessionRuntime { meta, transcript, workspace, terminal, agent }`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn switching_tabs_keeps_each_transcript_and_worktree() {
    let dir = tempdir().unwrap();
    let repo_a = temp_repo();
    let repo_b = temp_repo();
    let mut app = AppState::open(dir.path()).unwrap();
    let a = app.new_session(repo_a.path()).unwrap();
    let b = app.new_session(repo_b.path()).unwrap();
    app.switch(a).unwrap();
    app.active_session().unwrap().transcript.apply(&AgentEvent::TextDelta("in a".into()));
    app.switch(b).unwrap();
    assert_eq!(app.active_session().unwrap().meta.worktree, repo_b.path());
    assert!(app.active_session().unwrap().transcript.items().is_empty());
    app.switch(a).unwrap();
    assert_eq!(app.active_session().unwrap().transcript.items().len(), 1);
}

#[test]
fn reopening_reads_history_from_disk() {
    let dir = tempdir().unwrap();
    let repo = temp_repo();
    let mut app = AppState::open(dir.path()).unwrap();
    let i = app.new_session(repo.path()).unwrap();
    app.switch(i).unwrap();
    app.active_session().unwrap().log.append(&SessionEvent::User { text: "hi".into() }).unwrap();
    drop(app);
    let app = AppState::open(dir.path()).unwrap();
    assert_eq!(app.sessions().len(), 1);
    assert!(app.sessions()[0].worktree.exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-app sessions`
Expected: FAIL, `AppState` not found.

- [ ] **Step 3: Implement**

`AppState` owns a vector of `SessionRuntime` plus a chat channel from each agent and terminal. Switching swaps only which runtime the view reads; nothing reloads from disk and no process is restarted. A session whose worktree no longer exists is marked missing and refuses new turns. On start, sessions are read from the JSONL headers and the most recent is active. The dispatch path is also where plugin lifetime lands: when a plugin-backed tool call fails, call `PluginHost::release` with the registry so a trapped plugin's tools stop being offered instead of sitting there unusable.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`
Expected: PASS, all crates.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(app): session tabs wired to agent, terminal and files"
```

---

### Task 11: Wasm plugin host with tools

**Files:**
- Create: `crates/perch-plugins/Cargo.toml`, `crates/perch-plugins/src/lib.rs`, `crates/perch-plugins/src/host.rs`, `crates/perch-plugins/src/abi.rs`, `crates/perch-plugins/wit/perch-plugin.wit`, `crates/perch-plugins/fixtures/echo-plugin/{Cargo.toml,src/lib.rs}`, `crates/perch-plugins/fixtures/echo.wasm`, `scripts/build-plugin-fixtures.sh`
- Test: `crates/perch-plugins/tests/tools.rs`

**Interfaces:**
- Consumes: `ToolRegistry`, `Tool`, `ToolCtx`, `ToolOutput` from Task 7.
- Produces: `PluginHost::load(dir: &Path, registry: &mut ToolRegistry) -> Result<PluginHost>`; `PluginHost::len(&self) -> usize`; `PluginHost::names(&self) -> Vec<String>`; `PluginHost::release(&self, registry: &mut ToolRegistry) -> usize`, which drops a disabled plugin's tools through the registry and is idempotent; a `PluginTool` implementing `Tool` whose `call` invokes the guest with a 2s fuel-and-time bound and maps a trap to `Err` plus disabling that plugin.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_plugin_tool_registers_and_runs() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(Path::new("fixtures"), &mut registry).unwrap();
    assert_eq!(host.len(), 1);
    assert!(registry.specs().iter().any(|s| s.name == "echo"));
    let ctx = ToolCtx { root: tempdir().unwrap().path().to_path_buf() };
    assert_eq!(registry.call("echo", &json!({"text": "hi"}), &ctx).unwrap().text, "hi");
}

#[test]
fn a_trapping_plugin_is_disabled_and_its_tools_vanish() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(Path::new("fixtures"), &mut registry).unwrap();
    let ctx = ToolCtx { root: tempdir().unwrap().path().to_path_buf() };
    assert!(registry.call("explode", &json!({}), &ctx).is_err());
    assert!(!host.names().contains(&"explode".to_string()));
    assert!(!registry.specs().iter().any(|s| s.name == "explode"));
}

#[test]
fn a_plugin_cannot_read_outside_the_session_root() {
    let mut registry = ToolRegistry::builtin();
    PluginHost::load(Path::new("fixtures"), &mut registry).unwrap();
    let ctx = ToolCtx { root: tempdir().unwrap().path().to_path_buf() };
    let out = registry.call("read_outside", &json!({"path": "/etc/hosts"}), &ctx);
    assert!(out.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-plugins`
Expected: FAIL, `PluginHost` not found.

- [ ] **Step 3: Implement**

`perch-plugin.wit` declares a world exporting `tool-specs() -> list<tool-spec>`, `call(name, args-json) -> result<string, string>`, and importing only `log`. `PluginHost::load` reads each `.wasm` in the directory, instantiates it with wasmtime's component API and store limits (memory cap plus fuel), and registers one `PluginTool` per spec. A plugin tool whose name collides with an already-registered tool is rejected at load with an error naming both — never a silent overwrite of a builtin. Host IO is refused outright in this task — a plugin that wants a file must ask through a host import added in Task 12, so `read_outside` fails at the ABI boundary rather than by inspection. The fixture plugin is a `wasm32-wasip2` crate built by `scripts/build-plugin-fixtures.sh`, and its wasm is checked in so tests need no toolchain.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p perch-plugins`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(plugins): wasm host registering tools with trap isolation"
```

---

### Task 12: Declarative panels and the event stream

**Files:**
- Create: `crates/perch-plugins/src/panel.rs`, `crates/perch-ui/src/plugin_panel.rs`
- Modify: `crates/perch-plugins/wit/perch-plugin.wit`, `crates/perch-plugins/src/host.rs`
- Test: `crates/perch-plugins/tests/panels.rs`, `crates/perch-ui/tests/plugin_panel.rs`

**Interfaces:**
- Consumes: `PluginHost` from Task 11, `SessionEvent` from Task 1.
- Produces: `PanelModel { title: String, sections: Vec<PanelSection> }`; `PanelSection::{Text(String), Rows(Vec<String>), Button { id: String, label: String }, Input { id: String, label: String }}`; `PluginHost::panel_model(&self, plugin: &str) -> Option<PanelModel>`; `PluginHost::dispatch_event(&mut self, event: &SessionEvent)`; `panel_lines(model: &PanelModel) -> Vec<String>` in `perch-ui`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_panel_model_decodes_into_host_rendered_lines() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(Path::new("fixtures"), &mut registry).unwrap();
    let model = host.panel_model("counter").unwrap();
    assert_eq!(model.title, "Counter");
    assert_eq!(panel_lines(&model), vec!["count: 0".to_string()]);
}

#[test]
fn a_session_event_reaches_the_plugin_and_changes_its_panel() {
    let mut registry = ToolRegistry::builtin();
    let mut host = PluginHost::load(Path::new("fixtures"), &mut registry).unwrap();
    host.dispatch_event(&SessionEvent::TurnEnd { reason: "done".into(), tokens_in: 1, tokens_out: 2 });
    assert_eq!(panel_lines(&host.panel_model("counter").unwrap()), vec!["count: 1".to_string()]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p perch-plugins panels`
Expected: FAIL, `panel_model` not found.

- [ ] **Step 3: Implement**

The WIT world gains `panel() -> panel-model` and `on-event(event-json)`. The model is data only — rows of strings, buttons and inputs — so the host owns layout, theme and hit-testing and a plugin cannot draw over chrome. `panel_lines` flattens text and rows for the host's renderer and is what the UI test asserts, so panel content is proved without pixels. `dispatch_event` sends the serialised event to every loaded plugin; a plugin that traps is disabled by the same path as Task 11.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`
Expected: PASS, all crates.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(plugins): declarative panels and session events"
```

---

## After this plan

Task 11 and 12 close the plugin story the spec promises. After they land, the remaining work is polish: keybindings, themes, scrollback edge cases, packaging and the updater.
