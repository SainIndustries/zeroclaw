# M2 Exit — Bedrock ECS Credentials + TTL Cache

Branch: chore/upstream-sync-v0.7.3
Exit date: 2026-04-23

## Gate status

- [x] `AwsCredentials::from_ecs` fetches from `AWS_CONTAINER_CREDENTIALS_RELATIVE_URI` / `AWS_CONTAINER_CREDENTIALS_FULL_URI`
- [x] `AwsCredentials::resolve()` chain is env → ECS → IMDS
- [x] `CachedCredentials` wraps the SigV4 path with 50-minute TTL + double-checked-lock refresh
- [x] `BedrockProvider.resolve_auth()` uses the cache on every call for SigV4
- [x] Bearer-token path unchanged
- [x] `resolve_ecs_endpoint` has 3 unit tests; cache seeding has 1 unit test — all 4 pass
- [x] No regressions against M0 baseline (workspace: 3146 pass / 4 baseline fail, identical set)
- [x] Plan 1 gateway openai_compat::tests still green (5/5)

## What changed

Single file: `crates/zeroclaw-providers/src/bedrock.rs`

Additions:
- `resolve_ecs_endpoint()` free function returning `Option<(url, Option<auth_token>)>` from AWS_CONTAINER_CREDENTIALS_* env vars.
- `AwsCredentials::from_ecs()` async — fetches from the endpoint with a 3s timeout, optional Authorization header.
- `AwsCredentials::resolve()` chain: env → ECS → IMDS (was env → IMDS).
- `#[derive(Clone)]` on `AwsCredentials`.
- `const CREDENTIAL_TTL_SECS: u64 = 50 * 60;`
- `struct CachedCredentials { inner: Arc<RwLock<Option<(AwsCredentials, Instant)>>> }` with `new()` + `get()`.
- Imports: `std::sync::Arc`, `std::time::Instant`, `tokio::sync::RwLock`.

Modifications:
- `BedrockProvider` now has `cached_sigv4: Option<CachedCredentials>` alongside `auth` and `max_tokens`.
- `new()`, `new_async()`, `with_bearer_token()` all initialize `cached_sigv4` (Some for SigV4 path, None for bearer).
- 3 test helper sites also updated (`auth: None` → `cached_sigv4: None`).
- `resolve_auth()` short-circuits on bearer token, else consults `cached_sigv4.get().await`, else falls through to env/IMDS.

## What did NOT change

- SigV4 signing logic, Converse endpoint, message conversion, error handling.
- Bedrock `sanitize_empty_content_blocks` (already upstream).
- Any other crate.
- The Plan 1 fixture (`tests/fixtures/openai_compat_toolcall.sh`).
- Aura webapp code.

## Carry-forward for Plan 3 / Plan 5

- ECS container-credential endpoint is only exercised at runtime when the daemon runs in an ECS task. Plan 5 must verify a freshly-reprovisioned dev agent resolves credentials from the task role and makes a Bedrock call — that's the real end-to-end gate for ECS creds.
- Bearer-token path (`BEDROCK_API_KEY`) remains the preferred local-dev cred mechanism on macOS.
- `from_ecs` does not exist in upstream — stays a fork delta indefinitely. Consider upstreaming post-sync.
- The `BedrockAuth::SigV4` variant is now re-cloned from the cache on each `resolve_auth()`; signing overhead per request is unchanged vs upstream.

## Next

**Plan 3 (M3)**: feature-fork triage + port pass. Starts with M3.0 (adopts/drops: Telegram streaming, MCP Session-Id, Bedrock empty-block — mostly no-ops since upstream already has these; one-time allow-all bypass dropped in favour of `auto_approve=["*"]` at session boot). Then M3.1 triage of ~195 remaining FEATURE_FORK commits, and sub-phases M3.2–M3.9 for keep-ours features (post-turn memory, canary guard, shell approval rules, primary tool allow/denylist, HTTP credential_profile, Composio `3d746013` reapply, named families, deferred).
