# The agent loop: what a harness does, and what perch does

Status: reference. Read out of the OpenCode v2 mirror at `~/dev/monorepos/opencode` (commit
`1002a979f9`), not recalled.

## OpenCode's workflow, in the order a turn runs

| Stage | What happens | Where |
| --- | --- | --- |
| execution | one user prompt opens an execution; it holds the steps | `packages/core/src/session/execution.ts` |
| step | one model request and the tool calls it asks for; a step ends when the reply does | `session/runner/step.ts` |
| retry | a failed request is asked again on an exponential schedule; a partial stream is not | `model-retry.ts`, `session/runner/retry.ts` |
| step cap | steps are bounded; the last request removes the tools and asks for a written report | `session/runner/max-steps.ts` |
| compaction | a context-length rejection is retried at 85%, 70%, 50%, then 35% of the rejected size | `session/compaction.ts` |
| events | text, reasoning, tool input, tool progress, usage and status each stream as their own frames | `packages/schema/src/event-manifest.ts` |
| inbox | a prompt sent while a turn runs is queued and delivered after it, never interleaved | `session.ts` (`delivery`) |
| permissions | a tool asks once, and the answer can be `once`, `always` or `reject` | `permission.ts` |

Two properties decide the shape more than any code does:

1. **A step is not a request.** The model may ask for tools many times inside one step, and the
   turn's cost is the sum of every round.
2. **A stream that broke is not retried.** Only failures that arrive before any text are asked
   again; asking after text has been shown would print the answer twice.

### Retry, precisely

- 10 attempts, exponential from 2 s with the gap capped at 10 s, jittered: about a minute of
  waiting when every attempt fails (`model-retry.ts:22-36`).
- A provider's `retry-after` is honoured, capped at 15 minutes (`model-retry.ts:12-19`).
- A host that does not resolve or has no route is put on its own budget — capped at 30 s gaps for
  up to an hour — so a flaky link does not spend the general allowance (`model-retry.ts:38-61`).
- A timed-out attempt already cost minutes, so timeouts are retried at most three times
  (`model-retry.ts:65,82-86`).
- `x-should-retry: false` from the provider stops it regardless (`model-retry.ts:111-113`).

## What perch does now

- `crates/perch-agent/src/rounds.rs` — a turn is a loop of rounds, capped by `MAX_ROUNDS` (12).
- `crates/perch-agent/src/retry.rs` — 10 attempts, 2 s doubling to a 10 s cap, cancellable while
  waiting; rate limits, 408/409/425, 5xx, 529, transport failures and a stream cut before any
  event are retried; a bad key, a bad request and an empty model are not.
- `crates/perch-agent/src/open.rs` — the stream is opened with that policy, and only a failure
  before the first event is retried.
- `crates/perch-agent/src/report.rs` — when the rounds are spent, one more request with the tools
  removed asks for a written report, so the turn ends with words rather than an error.
- `crates/perch-agent/src/compact.rs` — a request the provider refuses as too long is answered
  with a smaller one rather than the same one again: the system message and the exchange being
  answered are kept, older exchanges are dropped, and the retry is asked at 85%, 70%, 50% then 35%
  of the size that was refused. Each drop is emitted as `AgentEvent::Compacted`, whose `notice()`
  reads "the conversation was too long, so perch dropped N older messages and asked again". The
  session log keeps every message; only the next request carries less.
- Approvals: `once`, `always` (for the session) and `reject`, in `dispatch.rs`; the same answer
  covers every call of that tool for the rest of the session.
- Cancellation, per-session model and per-session provider are handles shared with the agent.

## Not done yet, in the order they matter

1. **Streaming tool input and progress.** perch emits a tool call once its arguments are complete,
   so a long `bash` shows nothing until it ends. OpenCode streams the input and then the progress.
2. **Queued prompts.** A prompt sent mid-turn should queue and be delivered after it, not be
   refused or interleave.
3. **Usage per step.** The turn reports a total; a step's cost is what a person wants when a turn
   is expensive.
4. **Revert.** A checkpoint per step, so a turn can be undone without `git`.
