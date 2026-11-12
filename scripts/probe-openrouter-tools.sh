#!/bin/zsh
# Probes whether each candidate can call a tool, which is what a subagent needs before any other
# quality question matters. Prints whether a tool_call came back and its name.
set -u
AUTH="$HOME/.local/share/opencode/auth.json"
KEY=$(jq -r '.openrouter.key' "$AUTH")
HEADERS=$(mktemp)
trap 'rm -f "$HEADERS"' EXIT
printf 'Authorization: Bearer %s\nContent-Type: application/json\n' "$KEY" > "$HEADERS"
chmod 600 "$HEADERS"

TOOLS='[{"type":"function","function":{"name":"read_file","description":"Read a file","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}}}]'

for model in "$@"; do
    body=$(jq -nc --arg m "$model" --argjson t "$TOOLS" '{
        model: $m,
        messages: [{role: "user", content: "Read the file src/main.rs. Use the tool."}],
        tools: $t,
        max_tokens: 256
    }')
    out=$(curl -sS --max-time 60 https://openrouter.ai/api/v1/chat/completions \
        -H @"$HEADERS" -d "$body")
    name=$(print -r -- "$out" | jq -r '.choices[0].message.tool_calls[0].function.name // "none"' 2>/dev/null)
    args=$(print -r -- "$out" | jq -r '.choices[0].message.tool_calls[0].function.arguments // ""' 2>/dev/null | cut -c1-40)
    err=$(print -r -- "$out" | jq -r '.error.message // ""' 2>/dev/null | cut -c1-80)
    echo "$model | tool=$name | args=$args | $err"
done
