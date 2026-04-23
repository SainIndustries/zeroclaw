# v0.7.3 Baseline State

Created: 2026-04-23
Branch: chore/upstream-sync-v0.7.3 @ v0.7.3
Rustc: rustc 1.94.0 (4a4ef493e 2026-03-02)

## Build

- `cargo build --workspace` (default features): **GREEN** (1m22s)
- `cargo build --workspace --all-features`: **RED** on macOS — known platform limitation from `rppal` (Raspberry Pi GPIO) and `matrix-sdk`. These features are not relevant to Aura. Full build log: `/tmp/v073-baseline-build.log`.

Default-features build is our effective baseline for Aura work.

## Tests

`cargo test --workspace` (default features): 762 passed / 4 failed / 0 ignored (~3 min).

Known-failing tests at upstream v0.7.3 baseline (NOT regressions introduced by us):

```
test compatible::tests::flatten_system_messages_inserts_synthetic_user_when_no_user_exists ... FAILED
test compatible::tests::flatten_system_messages_inserts_user_when_missing ... FAILED
test compatible::tests::flatten_system_messages_merges_into_first_user ... FAILED
test compatible::tests::flatten_system_messages_merges_into_first_user_and_removes_system_roles ... FAILED
```

All 4 failures are in the `zeroclaw-providers::compatible::tests` module, related to OpenAI-compat system-message flattening. They exist on the upstream tag and do not block our port work.

## Cargo.lock deltas (pre-upstream-sync-snapshot vs v0.7.3)

- Full diff: `/tmp/v073-cargo-lock-diff.txt` (9,386 lines)
- Net: +393 added / -174 removed / +219 net deps

Aura-relevant dep version changes:

| Package | Pre-snapshot | v0.7.3 | Notes |
|---------|--------------|--------|-------|
| hyper | 1.8.1 | 1.9.0 | minor bump, non-breaking |
| clap | 4.5.60 | 4.6.0 | minor bump, non-breaking |
| uuid | 1.21.0 | 1.23.0 | patch bump |
| tokio | 1.50.0 | 1.50.0 | unchanged |
| axum | 0.8.8 | 0.8.8 | unchanged |
| serde | 1.0.228 | 1.0.228 | unchanged |
| reqwest | 0.12.28 | 0.12.28 | unchanged |
| anyhow | 1.0.102 | 1.0.102 | unchanged |
| thiserror | 1.0.69 | 1.0.69 | unchanged |
| tracing | 0.1.44 | 0.1.44 | unchanged |
| chrono | 0.4.44 | 0.4.44 | unchanged |

Notable non-Aura changes upstream:
- Added `reqwest` 0.13.2 alongside 0.12.28 (transitional)
- Added `tokio-tungstenite` 0.29.0 (WebSocket support)
- Removed `tokio-postgres` 0.7.16 and `tokio-postgres-rustls` 0.13.0 — Aura doesn't use these

## Conclusion

Baseline is GREEN on default features. No blockers to begin M1 (gateway adapter).
