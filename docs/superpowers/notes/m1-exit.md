# M1 Exit — Gateway Adapter Complete

Branch: chore/upstream-sync-v0.7.3
Exit date: 2026-04-23

## Gate status

- [x] `POST /v1/chat/completions` route registered in zeroclaw-gateway
- [x] Handler wraps `Agent::turn_streamed` and forwards `TurnEvent`s as OpenAI SSE
- [x] Translator unit tests pass (4/4) + auth test (1/1) = 5 tests in `openai_compat::tests`
- [x] `AURA_INTERNAL_SECRET` bearer shim accepted (with upstream paired-token fallthrough)
- [x] Streaming routes excluded from request TimeoutLayer
- [x] `tests/fixtures/openai_compat_toolcall.sh` passes end-to-end against a local daemon

## What changed

Files:
- **New**: `crates/zeroclaw-gateway/src/openai_compat.rs` — adapter module (~250 lines incl. tests)
- **New**: `tests/fixtures/openai_compat_toolcall.sh` — tool-calling SSE smoke fixture
- **New**: `docs/superpowers/notes/baseline-v073.md` — M0 baseline record
- **New**: `docs/superpowers/notes/m1-exit.md` — this file
- **Modified**: `crates/zeroclaw-gateway/src/lib.rs` — module reg, route reg, `AppState.aura_internal_secret` field, router split into fast/slow subrouters

Semantic behavior:
- `/v1/chat/completions` accepts OpenAI-shape requests, runs a fresh `Agent::from_config(...)` per request, streams `TurnEvent`s as OpenAI SSE deltas (role=assistant opener → content/reasoning/tool_calls/tool_result deltas → finish_reason + `[DONE]` sentinel).
- Auth: `AURA_INTERNAL_SECRET` bearer (if env set) OR any paired token validated by `PairingGuard::is_authenticated` (case-sensitive SHA-256 hash match).
- Streaming routes (`/v1/chat/completions`, `/ws/chat`, `/ws/canvas/{id}`, `/ws/nodes`) are in a separate subrouter with no request-wide timeout. Other routes keep the timeout.

## What did NOT change

- Upstream `ws.rs`, `api.rs`, provider code, agent loop — untouched.
- Aura webapp — no changes yet. The adapter accepts `AURA_INTERNAL_SECRET` unchanged.
- `Cargo.lock` deps — baseline v0.7.3 versions unchanged by M1.

## Smoke evidence

Passing end-to-end run recorded 2026-04-23 on macOS:
- Release build: `cargo build --release --workspace` (2m51s)
- Daemon started with `--config-dir /tmp/v073-smoke-config-dir`, port 18789, pairing disabled, autonomy=full.
- Fixture `tests/fixtures/openai_compat_toolcall.sh` passed all three conditions:
  1. at least one `tool_calls` delta (shell tool invoked with `echo hello`)
  2. at least one non-empty `content` delta (assistant wrapped the output)
  3. terminal `data: [DONE]`

## Environment notes (carry-forward for Plan 2 / Plan 5)

- **Model ID currently wired in fixture**: `bedrock/us.anthropic.claude-sonnet-4-6` (cross-region inference profile — required because `anthropic.claude-sonnet-4-20250514-v1:0` needs on-demand throughput not enabled on this account).
- **Bedrock creds on macOS**: upstream's Bedrock provider did not automatically resolve AWS creds from `~/.aws/credentials`; instead the daemon needs `BEDROCK_API_KEY=<bearer_token>` (we used `AWS_BEARER_TOKEN_BEDROCK`). This is consistent with the M2 finding that upstream has no ECS container-credential endpoint support — credential-chain handling on Bedrock is a Plan 2 concern.
- **IMDS timeout**: provider tried EC2 IMDSv2 and timed out before falling back to other creds sources. Another data point for M2.

## Next

**Plan 2 (M2)**: port Bedrock STS auto-refresh + ECS container-cred endpoint onto upstream `crates/zeroclaw-providers/src/bedrock.rs`. Fork commit to port: `d5cea40f fix(bedrock): auto-refresh AWS credentials before STS token expiry`.
