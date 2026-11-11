# Opening the window: where the 2.6 s went

The report was about 2.6 s between launching perch and anything being drawn, and the update check was already off the frame thread (its fake manifest takes 5 s and the window opens long before the offer arrives), so the fetch was not it. `crates/perch-app/tests/update/check.rs` was the test that had been paying the cost and calling it "the plugin engine".

Half of that guess was right and the half that was wrong was the expensive half.

## Per step, on this machine (debug build, macOS, M-series)

Measured with `std::time::Instant` around each step of `AppState::build`, on a data directory with one session and one 20 KB fixture plugin, no MCP servers.

| Step | Before | After |
| --- | --- | --- |
| `Sources::open` — wasm engine + every plugin + every MCP handshake | 1.05–12.2 ms with an empty plugin directory, **1.09 s with `counter.wasm`** | on the worker's thread |
| `Worker::start`, including the first published build | 250–420 µs + `published()` (≈50 ms) | 250–420 µs, publishes the built-ins and returns |
| `watch::watch` — the file watcher | **1.40–2.41 s** | **48 µs** |
| sessions (`headers` + `runtime` per session) | 52–59 µs | unchanged |
| **whole `AppState::build`** | **1.59–2.41 s** | **899 µs** |

The two numbers that matter: the plugin engine is a real second when a plugin is installed, and **the file watcher is the other 2.4 s**, which is where the "2.6 s" actually came from.

## The watcher is the one nobody suspected

`notify`'s FSEvents backend on macOS pays its cost inside `watcher.watch(path)`, not in building the watcher:

```
watch: watcher                                            128.6 µs
watch: watch("/private/var/…/T/.tmpmY6KoY", NonRecursive)  1.227 s
watch: watch("…/plugins", Recursive)                       1.180 s
```

One FSEvents stream restart per watched path — `notify 8.2.2`'s `run()` blocks until the run-loop thread has scheduled and started the stream (`~/.cargo/registry/src/index.crates.io-*/notify-8.2.2/src/fsevent.rs:393-498`, a `CFRunLoop` handshake per call). Boot watched two paths: the settings directory, and the plugin directory. Two seconds, before the first frame, for a watcher the first frame only ever asks "has anything changed?".

It is also **unbounded under load**: the same two-path install measured in the same test on this machine took **14.3 s** while other lanes were building. That ruled out simply moving the install to another thread — the window would open, but nothing would be watched for an arbitrary number of seconds afterwards, and a plugin dropped in that gap was lost (that is exactly how it showed up: `hot_reload_live` reported the built-ins and never the dropped plugin).

## What changed

1. **The tool sources open on the worker's thread** (`crates/perch-app/src/worker/mod.rs`, `worker/body.rs`). `Worker::start` takes the plugin directory and the server list instead of a built `Sources`, publishes `Snapshot::builtins()` — the built-ins every build starts from, which is a registry build with no engine and no server — and the thread opens the sources, publishes the result and only then starts draining the queue. `Worker::busy()` is true from the start, so frames keep coming and the snapshot is picked up by a pump rather than by a keypress.
2. **The watcher polls instead of streaming** (`crates/perch-app/src/watch.rs`). `PollWatcher` at 500 ms over the settings file and the plugin directory, with the same 200 ms debounce and the same `Watch::changed()` the frame already read. Two paths holding one settings file and a handful of components are not a directory tree worth a kernel stream: the sweep is two metadata reads and a short walk, it costs nothing to start (48 µs), and it watches from the first sweep rather than from whenever FSEvents finishes — so there is no window in which a save is missed, and no readiness handshake for the app or a test to wait on.
3. Nothing else moved: `Live`, the sessions, the update worker and the frame loop were already off the boot path (52 µs for a session).

## Why the built-ins and not an empty registry

An empty registry would have been the smallest change, and it makes a turn started a moment after the window opens go out with no tools at all — the model is asked a question it cannot act on. Publishing the built-ins instead costs one `Dispatcher::builtin_registry()` (part of the 1–12 ms above) and gives a window that can `read`, `write` and `bash` immediately, while a plugin's own tool is the only thing that arrives late.

A turn that reaches for a tool whose plugin is still loading does not panic on the list that is not there yet. It gets the registry's own refusal, which names what it could not find:

```
call before the tools arrive: no tool named echo
```

`crates/perch-app/tests/startup.rs::a_tool_that_has_not_arrived_is_named_in_the_refusal` holds that. `…::the_window_opens_before_its_plugin_tools_do` proves the order — the window has `read` and does not have the plugin's `echo`, and pumping a frame is what installs it — and `…::the_file_watcher_starts_without_waiting_on_the_filesystem` proves `watch()` returns in tens of microseconds and reports a save written straight after it.

A plugin engine that will not build is no longer a window that will not open: the failure is published as one line, reaches every session's transcript as a note, and the next save asks for the sources again. The same applies to an MCP server that never answers: it is named when its connect gives up, which is `perch_mcp::REQUEST_TIMEOUT` (30 s) after the window opened rather than 30 s before it — so a test waiting for that line waits for the transport's deadline, not for a frame.

## What only a window can confirm

- **Time to first presented frame.** Everything above is measured headless, up to the `Shell` that `init()` returns. The remaining hop — `rune::App::run` creating the window, laying out and presenting — is the native event loop and has no headless driver. The claim this work can make is about the app's own boot: 2.4 s of it is gone from the thread that opens the window.
- **Whether a 500 ms sweep feels immediate for a settings save.** A save is a person's action followed by reading a panel, and half a second is inside that loop, but "it felt instant" is not something a test can say.
- **Whether polling costs anything visible.** Two metadata reads and a walk of a handful of files twice a second: measurable with `fs_usage` on a real launch, not in a unit test.
