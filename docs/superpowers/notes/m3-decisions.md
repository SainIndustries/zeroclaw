# M3 Decisions Log

## One-time allow-all bypass — dropped (M3.0.a)

Our fork's `non_cli_allow_all_once` token / `bypass_non_cli_approval_for_turn` field (commit `5d38843f`) is NOT ported to v0.7.3. Upstream's `AutonomyLevel::Full` (approval/mod.rs:115) and `auto_approve = ["*"]` (approval/mod.rs:139) are functional replacements.

**Aura-side follow-up (Plan 5)**: `config-injector` emits `[autonomy] level = "full"` OR `[autonomy] auto_approve = ["*"]` for sessions previously using the bypass token.

**Zeroclaw-side impact**: none — upstream v0.7.3 never had the fork code; no deletion needed.
