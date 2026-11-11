#!/bin/sh
# Live probe: run the real agent loop against OpenRouter's free models.
#
# Resolves a key without printing it, builds the example once, then tries each free model
# in turn. A model that errors on tool calling is a failed attempt, not a crash: its status
# is printed and the next one is tried. Exits 0 on the first model that runs the loop.
#
# The key is read from ~/.local/share/opencode/auth.json under `.openrouter.key`, the same
# place the editor keeps it, and falls back to $OPENROUTER_API_KEY. It is handed to the
# probe in the environment, never on the command line, so it stays out of `ps`.
set -u

root=$(cd "$(dirname "$0")/.." && pwd) || exit 1
cd "$root" || exit 1

# One shared target directory for this repo, never a per-lane `target-*`: separate ones
# duplicated every dependency tree and took the disk to 2 GiB free.
target=${CARGO_TARGET_DIR:-$root/target}
if ! command -v jq >/dev/null 2>&1; then
	printf 'live-probe: jq is required to read the key file\n' >&2
	exit 2
fi

key=${OPENROUTER_API_KEY-}
if [ -z "$key" ]; then
	auth=${XDG_DATA_HOME:-$HOME/.local/share}/opencode/auth.json
	if [ -r "$auth" ]; then
		key=$(jq -r '.openrouter.key // empty' "$auth" 2>/dev/null || true)
	fi
fi
if [ -z "$key" ]; then
	printf 'live-probe: no OpenRouter key: %s has no .openrouter.key and OPENROUTER_API_KEY is unset\n' \
		"${XDG_DATA_HOME:-$HOME/.local/share}/opencode/auth.json" >&2
	exit 2
fi
printf 'key: resolved (not printed)\n'
printf 'target: %s\n' "$target"

if ! CARGO_TARGET_DIR=$target cargo build --example live_probe -p perch-agent; then
	printf 'live-probe: the probe does not build\n' >&2
	exit 2
fi
bin=$target/debug/examples/live_probe
if [ ! -x "$bin" ]; then
	printf 'live-probe: %s is missing after the build\n' "$bin" >&2
	exit 2
fi

for model in 'cohere/north-mini-code:free' 'nvidia/nemotron-3-ultra-550b-a55b:free' 'openrouter/free'; do
	printf '\n=== attempt: %s ===\n' "$model"
	OPENROUTER_API_KEY=$key "$bin" "$model"
	status=$?
	printf 'status: %s\n' "$status"
	if [ "$status" -eq 0 ]; then
		printf 'winner: %s\n' "$model"
		exit 0
	fi
done

printf '\nlive-probe: no free model ran the tool loop\n' >&2
exit 1
