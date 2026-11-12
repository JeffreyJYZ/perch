# Typing in the dock: where one keystroke goes

The report was "i type but the cursor kinda glides towards my typing", and before that "it feels smooth and dumb". Those are two different defects, and the measurements below separate them.

- The glide was motion: Rune animates a commit by default, and an element that exists in the last frame and moves in the next one travels there on a spring. The dock's caret moves one cell per echoed character, so every keystroke started a 300 ms spring and the caret slid into place.
- The latency is the frame loop: a keystroke writes to the pty and asks for nothing, so the echoed screen is drawn on the window's next poll, which is 0–16.7 ms later.

## The budget, hop by hop

| Hop | Measured | Where the number comes from |
| --- | --- | --- |
| Key event → pty write: channel send, thread wake, encode, `write_all`, `flush` | ≤ 82 µs | `cargo test -p perch-term --test typing -- --nocapture` |
| Shell echo → pty read → emulator feed → frame built and published | included above: p50 64.6 µs (`sh` tty echo), 58.5 µs (`zsh -f -i`) | same test |
| Frame build for a full 120×40 screen after 1.5 MB of output | 1.13 ms (dev profile, Zig `Debug`) | `cargo test -p perch-term --test terminal flood -- --nocapture` |
| Frame published → the window polls the channel | 0–16.7 ms | not measurable without a window; see below |
| Poll → install frame → view → layout → build | p50 1.51 ms, p90 1.58 ms, max 2.43 ms (dev, headless) | `cargo test -p perch-ui --lib shell::dock::budget -- --nocapture` |
| Build → present | 0–16.7 ms (vsync) | not measurable headless |
| Caret animating to its new cell | 300 ms spring, removed | `~/dev/experiments/rune/crates/rune-motion/src/tokens.rs:18` (`SETTLE`), which is `motion::DEFAULT` |

The terminal's own path is not the problem: 60–80 µs from the write to a whole new frame in hand, and that number includes the channel send, the thread wake, the emulator and the frame build. The two hops that cost milliseconds are the poll and the present, and both are the display's tick.

## What was ruled out, and how

- The pty reader batches nothing and drops nothing. `read_pty` (`crates/perch-term/src/session/thread.rs:119`) posts one message per `read`, and the terminal thread drains the queue into one frame per batch (`session/run.rs:57`). An echo is a single small read: one message, one frame.
- `pump` never loses an echo. It keeps the newest of up to eight queued frames (`shell/terminal/io.rs:74`, `:89`), so a burst collapses instead of queueing behind itself.
- Nothing buffers between the write and the child reading it. Every write is `write_all` then `flush` on the pty master (`session/run.rs:72`), and there is no TCP in the path. The round-trip number above is the evidence that nothing sits in a buffer.
- The shell is not slow to echo. `zsh -f -i`'s own line editor answers as fast as the kernel's tty echo, 58.5 µs against 64.6 µs p50, so the shell contributes no measurable delay to a single character. Its startup and prompt are a one-time cost before the first key.
- Every keystroke does rebuild the whole visible grid, on both threads, and at these sizes it is affordable: 1.13 ms for a 120×40 frame in the terminal thread's dev build, 1.5–2.6 ms for the install, view, layout and build on the UI side. That is the design — a `Frame` is a snapshot of the screen, because the emulator is neither `Send` nor readable from the UI thread — and it costs about a sixth of a display frame. If it ever stops being affordable the answer is a dirty-region frame, not a smaller grid.
- A key press did not ask for a repaint at all. It only wrote to the pty, so the echoed frame waited for the window's next poll: the 0–16.7 ms row above.
- The poll cadence. A live shell makes `Shell::next_wake` return `SHELL_TICK` (16 ms, `crates/perch-ui/src/shell/live.rs:18`), and Rune floors a timer to `TIMER_FRAME` (16.667 ms, `~/dev/experiments/rune/crates/rune/src/window/pacing.rs:195`), so the window polls the dock's channel at 60 Hz. Neither number could be observed in a test: they need the native event loop, which has no headless driver for timers. They are read from the source, not measured.

## What changed

1. The dock's grid lands its frames instead of animating them: `instant_changes()` on the `terminal-rows` column (`crates/perch-ui/src/shell/dock.rs`). This is the fix for the glide. A terminal is a grid of text, and every change inside it is a new picture of the same screen, not a layout change to travel across. It sits on the container rather than as `tx.instant()` in the key handler because the caret is moved by the poll that installs the echoed frame, and a handler's `tx.instant()` cannot reach a commit in a later frame. It also lands the wheel and a drag's selection, which change the same subtree. Regression test: `shell::dock::motion::an_echoed_character_lands_the_cursor_instead_of_gliding_it`.
2. A key press now reaches the shell at all: `Cmd::Key` encodes and writes on the terminal thread (`crates/perch-term/src/session/run.rs`, `handle.rs`, `command.rs`). The fire-and-forget path sent the press for encoding and dropped the reply, so Return, Tab, Backspace, Delete, Escape, Home, End, Page Up/Down, the arrows and F1–F12 were encoded and then discarded. Text went straight to the pty, which is why the terminal looked like it worked. Regression test: `crates/perch-term/tests/typing.rs::a_key_press_reaches_the_shell`.
3. A key's own commit lands at once: `tx.instant()` in `terminal_key` (`crates/perch-ui/src/shell/dock/input.rs`), for the one thing a key moves outside the grid — the header's words gain a width when the view pages into history, and its neighbour was sliding to meet them. Regression test: `crates/perch-ui/tests/dock.rs::a_page_key_lands_the_header_instead_of_sliding_it`.

## What is left, and why it was not done here

The remaining 0–16.7 ms is the window waking up to a frame that arrived while it slept. Rune has the seam for it: `AppHandle::wake` wakes the event loop so `poll` runs (`~/dev/experiments/rune/crates/rune/src/app.rs:648`), documented for exactly "a background thread that queued work for poll to drain".

The terminal half is in place: `TermSession::spawn_with_wake(cwd, cols, rows, wake)` calls `wake` after every published frame (`crates/perch-term/src/session/thread.rs`, `session/run.rs`), and `spawn` is that call with an empty callback. Regression test: `crates/perch-term/tests/wake.rs`.

What is missing is the call site: `crates/perch-ui/src/shell/actions.rs` opens the terminal with `TermSession::spawn`, and passing the window's waker there is one argument — `|wake| TermSession::spawn_with_wake(&cwd, cols, rows, wake)` with a closure that calls `AppHandle::wake`. That file belongs to the pane lane. With it, an echo is drawn on the display tick after it arrives instead of up to a tick and a poll later; without it, typing sits inside one 60 Hz frame, which is what every terminal that does not wake the compositor already costs.

Do not measure that hop by timing the app under a window and calling the difference latency: the poll, the commit and the present overlap the display's own cadence, and one sample says nothing about the worst case. The numbers above are the ones that are reproducible by the commands beside them.
