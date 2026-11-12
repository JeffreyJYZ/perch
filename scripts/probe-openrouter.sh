#!/bin/zsh
# Probes each candidate on OpenRouter with one tiny completion and prints one line per model.
# A catalogue listing proves nothing: a free ref can answer 200 with an empty completion, or 403
# by region, while still showing at zero price.
set -u
AUTH="$HOME/.local/share/opencode/auth.json"
KEY=$(jq -r '.openrouter.key' "$AUTH")
if [ -z "$KEY" ] || [ "$KEY" = "null" ]; then
    echo "no openrouter key in $AUTH"
    exit 1
fi
HEADERS=$(mktemp)
trap 'rm -f "$HEADERS"' EXIT
printf 'Authorization: Bearer %s\nContent-Type: application/json\n' "$KEY" > "$HEADERS"
chmod 600 "$HEADERS"

for model in "$@"; do
    body=$(jq -nc --arg m "$model" '{
        model: $m,
        messages: [{role: "user", content: "Reply with the single word: ok"}],
        max_tokens: 512,
        usage: {include: true}
    }')
    out=$(curl -sS --max-time 60 https://openrouter.ai/api/v1/chat/completions \
        -H @"$HEADERS" -d "$body" -w '\n%{http_code}')
    code=$(print -r -- "$out" | tail -1)
    json=$(print -r -- "$out" | tail -r | tail -n +2 | tail -r)
    text=$(print -r -- "$json" | jq -r '.choices[0].message.content // ""' 2>/dev/null | tr -d '\n' | cut -c1-60)
    tokens=$(print -r -- "$json" | jq -r '.usage.completion_tokens // "?"' 2>/dev/null)
    err=$(print -r -- "$json" | jq -r '.error.message // ""' 2>/dev/null | cut -c1-90)
    echo "$model | http=$code | out_tokens=$tokens | said=${text:-<empty>} | ${err}"
done
