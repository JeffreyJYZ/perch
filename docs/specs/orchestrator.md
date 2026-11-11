# Orchestrator: a lead session and the workers it dispatches

Status: contract for the next wave. The board is shipped (`crates/perch-board`); `sessions` and `ask` are not, and the lead loop below is what they are for. Nothing here is a scheduler, a queue, or a retry layer — see the last section before building on it.

## What the board already guarantees

- One file per project: `<root>/board/<slug>.jsonl`, append-only, one JSON object per line, no header. `<slug>` is the project directory's name plus a hash of its canonical path, so two projects never share a board.
- An entry: `{"id":u64,"at":u64,"from":"session","to":"session"?,"group":"label"?,"text":"...","files":["path"]?}`. `to`, `group` and `files` are omitted when absent.
- `id` is monotonic and sortable; `at` is unix milliseconds. A cursor is one number, so an id is all a reader needs to remember.
- `from` is written by the tool from the calling session, never from an argument, so a worker cannot post as the lead.
- The cursor lives at `<root>/board/<slug>.<session>.cursor`, one id per file, so "what I have not seen" survives a restart.
- A read returns entries in order, caps at 50, and says `[N more]` when it held some back.
- A malformed line is skipped: one bad byte does not make the board unreadable, and the cursor still passes it.
- A post is one `write_all` on an append-only file, so a crash loses at most the last line.

## The lead loop

- The lead lists the project's sessions, picks a worktree per unit, and dispatches one unit per session.
- Every dispatch names the group to post into: `group: "wave-1"`. The group is the wave's identity; the lead never parses worker text to tell one apart.
- A worker that finishes posts one entry: `post` = what it did, `files` = what it touched, `group` = `wave-1`. Its `from` is the worker's session id without it asking.
- A worker that cannot finish posts the same shape with `"blocked: "` or `"failed: "` in front of its text. Silence is not a result.
- The lead waits with `board_wait{match: "wave-1", timeout_ms: 30000}` and repeats until every unit of the wave has posted.
- A `board_wait` return is a read: each entry is handed to one reader once, and the lead's cursor is what makes the next wait show only what is new.
- `timed out` in a `board_wait` result is information, not failure: the lead names the units still outstanding and re-waits or reports.
- A worker proves it is alive with a heartbeat entry, `group: "wave-1"`, text starting `heartbeat:`, at least every 60 s of work. A heartbeat is cheap and a silent worker is indistinguishable from a dead one, so the interval is a contract, not a courtesy.
- A unit is dead when there is no heartbeat and no result for two heartbeat intervals, or when `sessions` reports the session gone. The lead posts one entry naming that unit and its session id, and stops waiting on it.
- A dead unit is re-dispatched as a new session with the same brief; the board is not asked to resume anything, because it holds messages and not state.

## Tool contracts for the next wave

### `sessions`

- Arguments: none.
- Output: one line per session in this project — `<session id> <state> <worktree> <title>` — state is one of `idle`, `running`, `waiting`, `dead`. Idle sessions first, then running, then dead.
- `dead` is a session whose process is gone but whose row is still known; `idle` is one that answered its last turn. A session that never ran is `idle` with no title.
- Failure: an empty project answers `no sessions`, not an error. A project whose session list cannot be read answers an error naming the reason, because "no sessions" and "cannot read" lead a lead to different decisions.

### `ask`

- Arguments: `session` (string, required), `text` (string, required), `wait_ms` (integer, default 120000, capped at 600000).
- Output: `#ask <session> <state>` on the first line, then the target's reply text capped at 8 KiB with `[truncated]` when it was longer. The reply is the target's assistant text for the turn this call started.
- Failure: an unknown session id is an error naming it; a session already mid-turn is `busy` and is not queued, so the lead decides whether to wait or pick another worker; a target that dies mid-reply is an error naming the session, and no partial reply is returned as though it were complete.
- `wait_ms` expiring answers `still running` plus whatever the target has said so far, not an error: the lead may wait again on the same session, and the board is where the durable result arrives.
- The call checks the caller's cancellation, so a cancelled lead turn does not leave a prompt parked on a worker.
- `ask` carries no board state. A worker's durable answer is the entry it posts; the reply exists to let a lead correct a brief without a round trip through the board.

### `board`

- Arguments: none to read. `post` (string) to append, with optional `to`, `group`, `files` (array of paths, capped at 32).
- Output: a read of what this session has not seen, one entry per line; `nothing new` when there is nothing, which is the answer most turns get and must stay one line. A post answers `posted #<id>`.
- Failure: an empty `post` is an error. An unreadable board directory is an error naming the io failure. A malformed line is not an error, because it is skipped.
- The token cost of a read is the entries plus one line; a wave of five workers returning 200 words each costs a paragraph, which is why results are terse.

## The task tool

- One tool, `task`, runs a child turn: `description` names it in the result line, `prompt` is the whole brief, and the answer is the child's final text.
- **No recursion, structurally.** The tool refuses when the registry it was handed already contains `task`, so a child that was given a registry without it cannot spawn grandchildren even if something misuses the API. A depth counter can be argued with; a missing tool cannot.
- **The child may only call tools that need no approval.** The registry is filtered to `!needs_approval()` before the child runs, so `read`, `list`, `grep`, `glob` and the web tools are available while `write`, `edit`, `bash` and `patch` are not. A subagent that silently writes files while the parent waits is the failure this rule exists to prevent, and the tool's own description says so, because the model should not plan a task that writes.
- **Cancellation is the parent's.** The delegate receives the parent's cancel token; a cancelled child returns a plain error saying it was cancelled, and the tool reports that as a tool result rather than a panic.
- **Caps live in the tool**: prompt and result are each capped at 64 KiB with the cut named. The round limit belongs to the delegate, which is the only part that knows how a turn is run.
- It does not stream: the parent sees one tool card and then a result. Streaming a child's thinking into the parent's transcript would need a second transcript, and that is not this feature.



- It is not a scheduler: it holds no ready set, no dependency graph and no notion of which unit may run next. The lead owns that.
- It is not a queue: two readers of the same board see the same entries, and nothing is claimed, leased or acknowledged.
- It does not retry: a post that never lands is never re-sent. A worker that dies before posting leaves no trace of its unit.
- It does not detect death: liveness is `sessions` plus heartbeats, and a worker that posts nothing looks exactly like one that is still thinking.
- It does not order across machines: one file, one filesystem, one project.
- It does not compact: the log is append-only and grows, and a read parses the whole file, so a board with tens of thousands of entries is a per-turn cost nobody measured.
- It does not deliver: a matching entry is returned to whoever reads first, so two leads sharing a board race rather than split work.
- It is not a permission boundary: `to` is an addressing hint enforced on read, and every session can read the file directly.
- `match` is a case-sensitive substring over text, group, sender and file paths, so `wave-1` also matches `wave-10`. Group names must be chosen so they cannot be prefixes of each other.
