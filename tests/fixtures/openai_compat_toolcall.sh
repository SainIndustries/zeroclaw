#!/usr/bin/env bash
# Smoke fixture for M1 — tool-enabled /v1/chat/completions over OpenAI SSE.
#
# Preconditions:
#   - zeroclaw daemon running on http://127.0.0.1:${ZEROCLAW_PORT:-18789}
#   - AURA_INTERNAL_SECRET="${AURA_INTERNAL_SECRET:-test-secret}" accepted by the adapter
#   - at least one tool that returns a deterministic value on a simple prompt
#     (e.g. the built-in `shell` tool with `echo hello`)
#
# This fixture MUST:
#   1. receive at least one tool_calls entry in a delta
#   2. receive at least one non-empty content delta
#   3. receive a terminal `data: [DONE]` line
#   4. exit 0 if all three conditions are met; exit 1 otherwise

set -euo pipefail

PORT="${ZEROCLAW_PORT:-18789}"
SECRET="${AURA_INTERNAL_SECRET:-test-secret}"

PROMPT="Run the shell command: echo hello, then tell me what it printed."

RAW=$(mktemp)
trap 'rm -f "$RAW"' EXIT

curl -sS -N \
  -H "Authorization: Bearer ${SECRET}" \
  -H "Content-Type: application/json" \
  -H "Accept: text/event-stream" \
  --data @- \
  "http://127.0.0.1:${PORT}/v1/chat/completions" <<JSON > "$RAW"
{
  "model": "bedrock/anthropic.claude-sonnet-4-20250514-v1:0",
  "stream": true,
  "messages": [
    {"role": "user", "content": "${PROMPT}"}
  ]
}
JSON

echo "=== raw SSE log ==="
cat "$RAW"
echo "==================="

# Condition 1: at least one tool_calls delta
if ! grep -Eq '"tool_calls":\s*\[' "$RAW"; then
  echo "FAIL: no tool_calls delta observed"
  exit 1
fi

# Condition 2: at least one non-empty content delta
if ! grep -Eq '"content":\s*"[^"]+"' "$RAW"; then
  echo "FAIL: no non-empty content delta observed"
  exit 1
fi

# Condition 3: terminal [DONE]
if ! grep -q '^data: \[DONE\]' "$RAW"; then
  echo "FAIL: missing terminal [DONE] sentinel"
  exit 1
fi

echo "PASS: tool_calls + content + [DONE] all present"
