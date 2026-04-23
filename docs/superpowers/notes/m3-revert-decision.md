# M3 Argenis-origin Port Reverts

Date: 2026-04-23
Trigger: post-port audit `aura-relevance-audit.md` found 7 committed ports were argenis-authored features Aura's config-injector never turns on. Forward-compatibility with upstream v0.7.3 > carrying ~2550 LOC of unused fork code.

## Reverted (7 commits)

| Original port SHA | Feature | LOC | Reason Aura doesn't need it |
|---|---|---|---|
| `758a6f00` | http credential_profile | 361 | No `credential_profile` reference in Aura |
| `1b7ec494` | autonomy non-cli exclusion | 73 | Aura explicitly sets `non_cli_excluded_tools = []` |
| `8815e61c` | post-turn memory extraction | 983 | Aura emits only `[memory] backend=sqlite, auto_save=true` |
| `575554a6` | canary guard | 323 | Aura has `level="full"`, no canary config |
| `a2159e18` | shell approval rules | 769 | Aura is full-autonomy, approvals disabled |
| `de98a450` | Discord approval buttons | 304 | No Discord webhook/cred-receiver in Aura |
| `0a34da53` | channels hot-reload delta | 22 | Aura hot-reloads via cred-receiver SIGTERM, not zeroclaw internals |

**Total reverted LOC**: ~2550 lines across 7 commits.

## Kept

- All Plan 1 Gateway work (`077ae723` wiring + `12b73966` TimeoutLayer + `591cc477` AURA_INTERNAL_SECRET + translator + skeletons + fixture)
- All Plan 2 Bedrock work (`4201e024` ECS endpoint + `1c5810ad` TTL cache + `5f744011` thread-through)
- `ec579cae` Composio entity_id security patch (Daniel-authored, Aura uses)
- Adopt-upstream decision records (`28c99030`, `36ffbdf5`)
- Triage research (`915ce91d`)
- Baseline + M1 + M2 exit notes

## Not started / dropped

- M3.5 primary tool filter port (argenis, never started)
- M3.8.a Discord buttons re-port (reverted)
- M3.8.b hot-reload delta re-port (reverted)
- M3.9 deferred ports (audit said 66 of 72 drop; ~6 minor items can be re-evaluated post-soak if needed)

## Net sync shape

The zeroclaw fork on `chore/upstream-sync-v0.7.3` is now **upstream v0.7.3 + gateway adapter + Bedrock ECS creds + Composio entity_id patch**. Forward-compatible. Minimal fork surface.

## Aura-side accommodations (Plan 5)

- config-injector: rename Bedrock slug `amazon-bedrock` → `bedrock`
- cred-receiver: use `PUT /api/config` instead of SIGTERM + SOUL.md rewrite
- NO `auto_approve = ["*"]` change needed — we reverted the approval-rules port, so approvals weren't gating non-CLI sessions anyway.
