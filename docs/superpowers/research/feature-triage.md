# FEATURE_FORK Triage — v0.7.3 sync

**Source**: /Users/danielhuynh/Documents/sain/aura/.tmp/zeroclaw-sync/keepers.md FEATURE_FORK section
**Target**: upstream v0.7.3 (tag + `upstream/master`)
**Date**: 2026-04-22
**Triager**: Claude Sonnet 4.6

---

## Summary

| Bucket | Count |
|---|---|
| KEEP-OURS | 72 |
| ADOPT-UPSTREAM | 47 |
| MERGE | 12 |
| DROP | 18 |
| DEFER | 55 |
| **Total triaged** | **204** |

---

## Already decided in spec §4 (excluded from this triage)

The following 10 commits were pre-decided in the sync spec and are excluded from verdict counts below:

| SHA | Subject | Decision |
|---|---|---|
| `09d7684cfa` | feat(memory): post-turn durable fact extraction | KEEP (M3.2) |
| `3702d224e9` | feat(security): canary token exfiltration guard | KEEP (M3.3) |
| `429ea06d69` | feat(approval): command-level shell approval rules | KEEP (M3.4) |
| `696a0c5432` | feat(agent): primary tool allowlist and denylist filtering | KEEP (M3.5) |
| `37d22440` / `4df1487e` | fix(http_request): credential_profile | KEEP (M3.6) |
| `3d746013` | fix(composio): entity_id | KEEP (M3.7) |
| `135e4ed730` | feat(telegram): StreamMode::On native draft streaming | ADOPT-UPSTREAM (upstream has `118cd539` — richer: `stream LLM responses to Telegram via draft message edits` with the same StreamMode enum) |
| `1162df77` | fix(mcp): Session-Id | ADOPT-UPSTREAM (already upstream) |
| `2cff3e0f85` | fix(bedrock): empty content block | ADOPT-UPSTREAM (upstream superset) — note: this is in AURA_GLUE too |
| `5d38843f` | fix(agent): one-time bypass | DROP (Aura uses auto_approve) |

---

## Triage verdicts

### KEEP-OURS

Features with no upstream equivalent that are product-relevant or carry meaningful functionality.

| SHA | Subject | Reasoning | Port to |
|---|---|---|---|
| `91d8abf723` | feat(observability): add labeled WATI webhook auth failure metric | WATI is Aura-specific WhatsApp integration; upstream has no labeled auth-failure metric for WATI. | src/observability |
| `0b5665ad9b` | feat(agent): add adaptive load balancing for teams and subagents | No upstream equivalent for adaptive load balancing across team/subagent pools. Needed for multi-user hosted product. | src/agent |
| `49384b1678` | feat(agent): intelligent team/subagent orchestration with hot config | Paired with above; hot-config team orchestration is our product feature for Aura multi-agent. | src/agent |
| `3fb11acade` | feat(memory): boost core memories during context retrieval | Upstream has time-decay (`a9ffd389`) but no core-memory boost during retrieval. This pairs with our post-turn memory. | src/memory |
| `2052c720cc` | feat(memory): flush durable facts before compaction | No upstream equivalent; ensures post-turn facts survive context compaction. Dependent on M3.2. | src/memory |
| `cc9ff1820b` | feat(autonomy): exclude process by default for non-cli channels | Security default; upstream does not exclude `process` tool from non-CLI channels. Reduces blast radius in hosted. | src/channels |
| `09d32dcd79` | feat(security): add context-aware command allow rules | Extends M3.4 shell approval with per-context allow rules. No upstream equivalent for context-scoped allows. | src/security |
| `3f6e192b14` | feat(channel): add native Discord approval buttons and interactions | Discord-native interactive approvals via buttons. Upstream has only text-based approval. | src/channels |
| `9fb0e30dac` | feat(channels): hot-reload runtime tool-loop and memory defaults | Per-channel hot-reload of tool-loop settings. Upstream config reload (`dd147dd0`) is global-only. | src/channels |
| `f3c82cb13a` | feat(tools): add xlsx_read tool for spreadsheet extraction (#2338) | Confirmed absent from upstream tools tree (`src/tools/`). Aura agents need spreadsheet read for business workflows. | src/tools/xlsx_read.rs |
| `a1d51b6454` | feat(agent): add ProgressTracker for in-place tool progress updates | No upstream equivalent (`ProgressTracker` not found upstream). Drives streaming progress UX in Aura dashboard. | src/agent |
| `bfacba20cb` | feat(config): add ProgressMode enum for streaming channel draft updates | Config schema backing ProgressTracker; `ProgressMode` absent from upstream. | src/config |
| `84b43ba4b2` | feat(memory): add reindex command to rebuild embeddings [CDV-28] | Upstream search engine (`0e7f501f`) adds FTS5/vector but does not expose a user-facing `reindex` CLI command. Needed for migration support. | src/memory |
| `4d195be713` | feat(channel): add BlueBubbles iMessage channel | No upstream BlueBubbles channel. Aura uses this for iMessage relay (see BLOOIO notes — our iMessage is Blooio, but BlueBubbles was predecessor; channel code is unique). | src/channels/bluebubbles.rs |
| `0253752bc9` | feat(memory): add observation memory tool | `observation` keyword absent from upstream. Distinct from standard `memory_store`; gives agents semantic tagging layer. | src/tools/memory_observe.rs |
| `cb1cd14cbb` | feat(tools): add pptx_read tool for PowerPoint text extraction | Confirmed absent from upstream tools tree. Business doc extraction needed. | src/tools/pptx_read.rs |
| `9ecb8dffa6` | feat(memory): add sqlite_journal_mode config for shared filesystem support | No upstream `sqlite_journal_mode` config. Needed for EFS-backed multi-tenant agent storage on AWS. | src/config |
| `8c0be20422` | feat(providers): add quota_metadata to ChatResponse across all providers | Upstream has rate-limit guards but no `quota_metadata` on ChatResponse. Feeds our quota monitoring system. | src/providers |
| `247d89e39e` | feat(providers): implement quota monitoring system with CLI and agent tools | No upstream quota monitoring. Product feature for Aura's budget enforcement. | src/tools/quota_tools.rs + src/providers/quota_*.rs |
| `d2b0338afd` | feat(providers): implement circuit breaker with provider health tracking | No upstream circuit breaker pattern (`d31f2c2d` is loop detection, not circuit breaker). Critical for multi-provider reliability in hosted product. | src/providers/health.rs |
| `a029c720a6` | feat(security): add safety heartbeat reinjection with cadence fixes | No upstream safety heartbeat reinjection. Distinct from `heartbeat` scheduler; this rejects policy-bypassing turns. | src/security |
| `51ad52d0e8` | security: harden sensitive I/O and outbound leak controls | Our fork-specific I/O leak controls for hosted environment; upstream has some hardening but not this exact boundary enforcement. | src/security |
| `87fa327e0d` | feat(telegram): add ack_enabled option to control emoji reactions | `ack_enabled` absent from upstream; upstream has `ack_reactions` in config but not a bool enable gate. Product-tunable. | src/channels/telegram.rs |
| `df6f7455e7` | feat(tools): add docx_read tool for DOCX text extraction | `docx_read` absent from upstream tools tree. Business doc extraction needed. | src/tools/docx_read.rs |
| `d943f9c28c` | feat(tools): add bg_run — background tool execution with security hardening | `bg_run` absent from upstream. Allows agents to run long tasks without blocking the response channel. | src/tools/bg_run.rs |
| `f4d06a3a73` | feat(memory): add optional cortex-mem backend profile and bridge | No upstream `cortex-mem` backend; upstream memory is SQLite+vector only. Future external memory integration point. | src/memory |
| `da62bd172f` | feat(tools): add user_agent config and setup_web_tools wizard step | `user_agent` config for http tool and web wizard absent from upstream. Needed for compliant outbound requests. | src/tools/web_access_config.rs |
| `856afe8780` | feat(coordination): deep-complete agent coordination message bus | No upstream agent coordination bus; upstream has shared iteration budget (`21bd251d`) but not a full message bus. | src/coordination |
| `a2f1f09364` | feat(economic): add TaskClassifier for BLS occupation-based task valuation | `TaskClassifier` absent from upstream; unique economic layer for Aura. | src/economic |
| `b238e8fd5e` | feat(config): add economic agent configuration schema | Backing schema for economic module; absent from upstream. | src/config |
| `9798b34f8d` | feat(economic): add ClawWork economic tracking module | `ClawWork` absent from upstream; Aura-specific task valuation. | src/economic |
| `498fca9d08` | feat(memory): add sqlite+qdrant hybrid backend | Upstream memory is SQLite-only in its public surface; no upstream sqlite+qdrant hybrid. | src/memory |
| `0129b5da06` | feat(onboard): add hybrid sqlite+qdrant memory option in wizard | Onboarding extension for above; absent upstream. | src/onboard |
| `a09f146145` | feat(security): add role-policy and otp challenge foundations | Upstream has OTP config fields (`981a93d9`) but no role-policy engine. Distinct from upstream OTP config. | src/security |
| `7aee6d9dc7` | feat(security): add role-policy and otp challenge foundations (duplicate/iteration) | Same as above — later iteration; keep latest. | src/security |
| `bfe3e4295d` | feat(security): add opt-in perplexity adversarial suffix filter | `perplexity` adversarial suffix filter absent from upstream. Novel security layer. | src/security |
| `970ef57f21` | feat(security): add aho-corasick and entropy leak heuristics | `aho-corasick` entropy heuristics absent from upstream. Extends canary/leak detection. | src/security |
| `d63a6a8ceb` | feat(security): unify URL validation with configurable CIDR/domain allowlist | Upstream has per-tool SSRF guards but no unified CIDR/domain allowlist config. | src/tools/url_validation.rs |
| `fbb3c6aee0` | feat(tools): add apply_patch tool and update tests | `apply_patch` absent from upstream tools tree. Useful for LLM patch workflows. | src/tools/apply_patch.rs |
| `067eb8a188` | feat(tools): add sub-agent orchestration (spawn, list, manage) | Upstream has shared iteration budget but not spawn/list/manage subagent tools. | src/tools/subagent_*.rs |
| `6064890415` | feat: goals engine, heartbeat delivery, daemon improvements, and cron consolidation | Goals engine absent from upstream; our `src/goals/` directory is unique. | src/goals |
| `16961bab84` | feat(channels): hide internal tool progress unless explicitly requested | Upstream `show_tool_calls` config (`f900d707`) suppresses notifications, not same as progressive reveal. | src/channels |
| `a9e8526d67` | feat(channels): add unified group-reply policy and sender overrides | No upstream unified group-reply policy; upstream has per-channel group configs. | src/channels |
| `4043056332` | feat(cost): enforce preflight budget policy in agent loop | Upstream has cost tracking but no preflight budget enforcement before agent loop runs. | src/cost |
| `479b6da4ce` | feat(cost): wire provider token usage to cost tracking (#2111) | Upstream wires cost tracking (`e407415a`) but ours predates it and uses a different path; evaluate for MERGE on port. Listed KEEP pending M3.9 review. | src/cost |
| `6662601a6c` | feat(agent): add result-aware loop detection for tool-call loop | Upstream has basic loop detection (`d31f2c2d`) but not result-aware variant. | src/agent |
| `cfe1e578bf` | feat(security): add and harden syscall anomaly detection | No upstream syscall anomaly detection; novel security layer. | src/security |
| `268b01fcf0` | hardening(security): sanitize upstream error bodies across channels | Upstream sanitizes some error paths but not a unified cross-channel error body sanitizer. | src/security / src/channels |
| `495d7717c7` | hardening(logging): sanitize channel API error bodies | Same area as above; keep for the logging-side implementation. | src/channels |
| `346f58a6a1` | hardening: strengthen tool policy enforcement and sandbox defaults | Upstream has sandbox (`9cc74a26`) but our hardening adds additional defaults not yet upstream. | src/security |
| `2ecfa0d269` | hardening: enforce channel tool boundaries and websocket auth | WebSocket auth boundary enforcement absent from upstream in this form. | src/channels / src/gateway |
| `883f92409e` | feat(channels): add query classification routing with logging | Upstream wires query_classification (`1a5d91fe`) but our version adds logging. | src/channels |
| `aac87ca437` | feat(provider): add reasoning level override | Upstream adds thinking/reasoning level per-message (`b67be417`) but our `reasoning_level` config alias differs from `efdd40787c`; keep for Aura's compat alias. See MERGE note. | src/providers |
| `3a38c80c05` | feat(config): add model_support_vision override | `model_support_vision` absent from upstream; per-model vision override needed for Ollama multi-model installs. | src/config |
| `ed67184c7a` | feat(tools): add inter-process communication tools | Upstream has no IPC tools (`agents_ipc.rs` absent). | src/tools/agents_ipc.rs |
| `817f783881` | feat(agent): inject shell allowlist policy into system prompt | No upstream equivalent; pairs with M3.4 approval rules. | src/agent |
| `d6d32400fa` | feat(tool): add session-scoped task_plan tool for multi-step work tracking | `task_plan` absent from upstream. Structured task decomposition for agents. | src/tools/task_plan.rs |
| `56ffcd4477` | feat(tool): add background process management tool (spawn/list/output/kill) | `process.rs` absent from upstream tools; distinct from `bg_run`. | src/tools/process.rs |
| `d5fe47acff` | feat(tools): wire auth_profile + quota tools into agent loop | Auth profile wiring unique to our fork; pairs with credential_profile (M3.6). | src/tools/auth_profile.rs |
| `f0a5bbdb1b` | feat(http_request): add env credential profiles and onboarding guards | Earlier iteration of M3.6 credential_profile; keep for onboarding guard component. | src/tools/http_request.rs |
| `1fcf2df28b` | feat: harden non-CLI approval governance and runtime policy sync | Runtime policy sync absent from upstream; extends M3.4 approval rules. | src/approval |
| `1ad2d71c9b` | feat(approval): add one-time all-tools non-cli approval flow | Non-CLI all-tools approval absent from upstream (upstream has CLI-only `c301b1d4`). | src/approval |
| `46d087eb8f` | refactor(update): remove unused ErrorKind import | Minor cleanup that pairs with self-update work; DROP candidate but safe to keep — actually DROP (see below). | — |
| `cc8aac5918` | feat: channel improvements (Lark rich-text, WhatsApp QR, draft config) | Lark rich-text and WhatsApp QR are not in upstream. Draft config improvements are ours. | src/channels |
| `b3b5055080` | feat: replay custom provider api mode, route max_tokens, and lark image support | Lark image support + replay api mode for Aura's custom provider; partially upstream but assembled differently. | src/channels / src/providers |
| `362a81a3e5` | refactor(plugins): add validation profiles with strict runtime defaults | Plugin validation profiles absent from upstream in this form; pairs with our plugin system. | src/plugins |
| `60b73b6cd3` | feat(slack): add socket mode listener fallback | No upstream socket mode fallback; upstream uses events API only. | src/channels/slack.rs |
| `61d538b6d6` | feat(slack): support listening on multiple configured channel IDs | No upstream multi-channel-ID slack listener. | src/channels/slack.rs |
| `0d68992fb7` | feat(session): Add channel session persistence | Upstream has SQLite session backend (`9ba5ba56`) but our channel-level session persistence differs from the upstream trait abstraction. Evaluate for MERGE — listed KEEP for now. | src/channels |
| `cc0bc49b2f` | feat(channel): add napcat support for qq protocol | `napcat` absent from upstream; our `src/channels/napcat.rs` is unique. | src/channels/napcat.rs |
| `5aac1af065` | feat(channel): support onebot aliases for napcat config | Extension of napcat channel above. | src/channels |
| `54dd7a4a9b` | feat(qq): add webhook receive mode with challenge validation | Our QQ channel adds webhook receive mode; upstream QQ (`a047a0d9`) is rich-media focused but lacks our webhook challenge validation. | src/channels |

---

### ADOPT-UPSTREAM

Features where upstream has an equivalent (often richer) implementation. We should drop ours and use upstream.

| SHA | Subject | Upstream equivalent | Reasoning |
|---|---|---|---|
| `135e4ed730` | feat(telegram): StreamMode::On native draft streaming | `118cd539` `feat(channel): stream LLM responses to Telegram via draft message edits` | Upstream predates our commit and has the same StreamMode enum with `off/partial/block` plus `draft_update_interval_ms`. Adopt upstream's full implementation. |
| `fe3556da58` | feat(file_edit): add whitespace-flexible fallback matching | `9ff86c37` `fix(tools): reject empty old_string in file_edit` + `34ec7889` `feat(tools): add file_edit tool` | Upstream file_edit is the base; whitespace-flex is a nice-to-have already partially covered by upstream's fuzzy path. Verify upstream before porting. |
| `2630486ca8` | feat(providers): add StepFun provider with onboarding and docs parity | `f8a57e1e` `feat(provider): add Alibaba Coding Plan support` + upstream has OpenAI-compatible fallback | StepFun uses OpenAI-compatible API; upstream's `compatible.rs` covers it. Our custom provider file is redundant. |
| `f3f44c48f4` | feat(providers): integrate Volcengine ARK and SiliconFlow | `fc8ed583` `feat(providers): add VOLCENGINE_API_KEY env var` + `c7731707` (SiliconFlow gaps) | Both providers are now upstream with at least equal coverage. |
| `684503f5fc` | feat(onboard): add GitHub Copilot to interactive wizard | `9f245539` `feat: github copilot onboarding` (upstream is more recent and complete) | Upstream has a richer implementation including device-flow auto-prompting. |
| `250a2247cd` | feat(hardware): add gpio_read/gpio_write tool implementations | `71e89801` `feat(hardware): add RPi GPIO, Aardvark I2C/SPI/GPIO, and hardware plugin system` | Upstream hardware is a full workspace crate with GPIO, I2C, SPI. Ours is a subset. |
| `8eeea3fca1` | feat(hardware): add device registry and serial transport foundations | Same as above — `71e89801` + `3b72d525` `feat(workspace): extract zeroclaw-hardware crate` | Upstream has a dedicated `crates/zeroclaw-hardware`. Adopt the workspace crate. |
| `276ff7bd42` | feat(channels): add matrix integration for sovereign communication | `34dc66c9` `feat(matrix): mention_only filtering, enhanced media handling` + `e91806c1` | Upstream has a mature Matrix channel with mention_only, media, streaming. Our addition predates theirs but is now superseded. |
| `46c9f0fb45` | feat(matrix): add mention_only gate for group messages | `b7f3d0c9` `feat(matrix): add mention_only config for group room filtering` | Upstream has this exact feature. Drop ours. |
| `f1ca0c05fd` | feat(lark): add mention_only group gating with bot open_id auto-discovery | `3b2009f1` `feat(lark): add mention_only group gating with bot open_id auto-discovery` | Upstream has the identical commit (same feature — likely cherry-picked from us or developed in parallel; adopt upstream version for cleaner history). |
| `7307aab103` | feat(tools): add Tavily provider and API-key round-robin | `56c7d605` `feat: add Tavily as web search provider option` | Upstream has Tavily integration. Our version adds round-robin, which may be a MERGE candidate; defaulting ADOPT since upstream round-robin likely exists via `web_search_provider_routing`. |
| `b5292f54aa` | feat: plugin system | `1341cfb2` `feat(plugins): add Extism dependency, feature flag, and plugin module skeleton` + `c9b7a122` | Upstream has a WASM plugin system via Extism. Ours predates it; adopt upstream's Extism-based implementation. |
| `49a520df3e` | feat(plugins): execute wasm tools/providers via host abi bridge | `67edd2bc` `fix(plugins): integrate WASM tools into registry, add gateway routes and tests` | Upstream has WASM tool execution via Extism. Adopt. |
| `1d6afe792b` | feat(plugins): scaffold wasm runtime and wire core hook lifecycle | `c857b64b` `feat(plugins): add Extism dependency, feature flag, and plugin module skeleton` | Upstream scaffold. Adopt. |
| `05d36862c5` | feat(plugins): add hot-reload state and activate observer bridge | Upstream hot-reload config (`740eb17d`) + plugin registry | Upstream handles this more cleanly in the new crate structure. |
| `9b0aa53adf` | feat(plugins): enforce runtime limits and add echo plugin example | Upstream plugin crate has runtime limits | Adopt upstream Extism-based limits. |
| `8180e7dc82` | feat(skills): add WASM skill engine with secure registry install | Upstream has SkillForge (`35b63d6b`) + `a47a9ee2` (ClawhHub skill installer) | Upstream skill engine covers this. |
| `604f64f3e7` | feat(runtime): add configurable wasm security runtime and tooling | Same upstream Extism/WASM plugin system | Adopt upstream. |
| `163f2fb524` | feat(wasm): harden module integrity and symlink policy | Upstream WASM security | Adopt upstream's security model. |
| `4f8c9d2066` | feat(mcp): add external MCP server support on main | Upstream has `mcp_client.rs`, `mcp_tool.rs`, MCP transport in tools tree | Upstream MCP is comprehensive. Our "main" branch addition is superseded. |
| `6186b34903` | refactor(mcp): use schema paths to avoid config re-export conflicts | Upstream has clean MCP config in workspace crate | Adopt upstream workspace split. |
| `6ed7248d65` | refactor(config): split mcp re-exports to avoid main merge conflict | Same as above | DROP/ADOPT — merge conflict workaround no longer needed with upstream workspace structure. |
| `87ac60c71d` | feat(tools): Use system default browser instead of hard-coded Brave Browser (#1453) | `77a3b39f` — identical commit upstream (same SHA logic; this is the upstream commit cherry-picked to our fork) | This is literally the same commit that landed upstream. Pure ADOPT — nothing to port. |
| `e52a518b00` | feat(channels): add /new command to clear conversation history (#1417) | Upstream has `/new` in channel handling | Likely already upstream; verify before porting. |
| `04e8eb2d8e` | feat(models): add list, set, and status subcommands | `ef47cf14` `feat(models): add list, set, and status subcommands` (same commit in upstream) | Upstream has this exact feature. |
| `8a1409135b` | feat(config): warn on unknown config keys (#1410) | `d3c8ff6a` `feat(config): warn on unknown config keys to prevent silent misconfig (#1410)` — identical PR | Upstream has this. |
| `359cfb46ae` | feat(agent): inject current datetime into every user message | `baa01dab` `feat(agent): inject current datetime into every user message` | Upstream has the identical feature. |
| `b36dd3aa81` | feat(logging): use local timezone for log timestamps | `ee396986` `fix(observability): use local timezone for runtime_trace timestamps` | Upstream has timezone-aware logging. |
| `55ded3ee16` | feat(agent): log query classification route decisions | `055507bd` `feat(agent): log query classification route decisions` | Identical upstream. |
| `7d6d90174f` | feat(channel): use DingTalk Open API for sending messages | `9463bf08` `feat(channels): add DingTalk channel via Stream Mode` (upstream) | DingTalk is upstream. |
| `3a4e55b68d` | feat(providers): auto-refresh expired Gemini OAuth tokens in warmup | `0d667752` `fix(gemini): fix OAuth provider for cloudcode-pa internal API` | Upstream handles Gemini OAuth refresh. |
| `b721754ead` | feat(codex): add websocket-first transport selection | Upstream Codex provider has `openai_codex.rs` | Upstream Codex provider handles transport; evaluate on port. |
| `b8de8ce8b9` | feat(transcription): support config-level api_key | `756c3cad` `feat(transcription): add LocalWhisperProvider for self-hosted STT` | Upstream transcription has API key config. |
| `f8eef67a03` | feat(whatsapp-web): transcribe voice messages via Groq Whisper | `2eaa8c45` `feat(whatsapp-web): add voice message transcription support` | Upstream has WhatsApp voice transcription. |
| `1177a83e4a` | feat(telegram): register bot commands with setMyCommands on startup | Upstream telegram has bot command registration | Upstream covers this. |
| `63fcd7dd54` | feat(telegram): support custom Bot API base_url | Upstream telegram config supports base_url | Adopt upstream. |
| `955c572c02` | feat(tools): add Chrome/Firefox/Edge support to browser_open tool | `c9d76780` `fix(security): harden redirect/browser_open` | Upstream browser_open exists; multi-browser support likely present in upstream version. |
| `3a4e55b68d` | feat(providers): auto-refresh expired Gemini OAuth tokens | Upstream Gemini OAuth (`0d667752`) | Adopt upstream. |
| `1ad5416611` | feat(providers): normalize image paths to data URIs in OpenAI Codex | Upstream Codex provider handles image normalization | Adopt upstream. |
| `12a3fa707b` | feat(providers): add vision support to OpenAI Codex provider | `15d84b26` `fix(copilot): support vision via multi-part content messages` | Upstream handles vision across providers. |
| `a25ca6524f` | feat(skills): support front-matter metadata and always-inject skills (#2248) | `8a4da141` `fix(skills): inject skill prompts and tools into agent system prompt` | Upstream skill injection covers this. |
| `a851d1bd2f` | feat(skills): add configurable script-file audit override | `191192a1` `add configurable allow_scripts audit option` | Upstream has this option. |
| `36d5d2f3f8` | feat(skills): seed bundled zeroclaw skill on startup | Upstream SkillForge auto-seeds | Adopt upstream SkillForge behavior. |
| `bde9d45ead` | feat(cron): add lark and feishu delivery targets | `3eca2668` `fix(channel,provider): add lark/feishu cron delivery` | Upstream has lark/feishu cron delivery. |
| `6500f048bc` | feat(email): add IMAP ID extension support | `5d9e8705` `refactor(channel): replace hand-rolled IMAP with async-imap IDLE` | Upstream refactored IMAP wholesale; adopt their async-imap approach. |
| `390373dbcb` | feat(cli): add self-update command | Upstream has self-update infrastructure (release management) | Adopt upstream update flow. |
| `13469f0839` | refactor(telegram): remove redundant else in startup probe | Trivial refactor; upstream telegram is cleaner post-v0.7.3 | DROP if upstream is clean; otherwise trivial to re-apply. |

---

### MERGE

Features where both we and upstream have partial implementations that need reconciliation.

| SHA | Subject | Upstream equivalent | Merge plan |
|---|---|---|---|
| `efdd40787c` | feat(config): add deprecated runtime reasoning_level compatibility alias | `b67be417` `feat(agent): add thinking/reasoning level control per message` | Upstream uses `thinking_level`; our fork uses `reasoning_level` as an alias. Port the alias to point to upstream's `thinking_level` field. |
| `aac87ca437` | feat(provider): add reasoning level override | Same as above | Same merge: wire our reasoning_level → upstream thinking_level. |
| `0d68992fb7` | feat(session): Add channel session persistence | `9ba5ba56` `feat(sessions): add SQLite backend with FTS5, trait abstraction, and migration` | Upstream has a full session trait + SQLite backend. Our channel-level persistence differs. Reconcile by adopting upstream trait and migrating our channel wiring. |
| `762ca25e19` | feat(channels): add chat-scoped ACK rules and simulation aggregates | `c4c52368` `feat(slack): reaction-based cancellation and finalize_draft thread fix` | Upstream has reaction-based ACKs for Slack; our fork generalizes this to all channels with simulation. Merge our cross-channel ACK config into upstream's reaction framework. |
| `f594a233b0` | feat(channels): enrich ack reaction policy with regex sampling and simulate | Same upstream reaction system | Same merge as above. |
| `8583f59066` | feat(channels): add configurable ack reactions and channel ack config tool | Same upstream reaction system | Same merge as above — these three ACK commits form one feature to reconcile. |
| `2d91536f92` | feat(routing): support hint default_model during startup | Upstream has `model_routes` and routing config | Upstream model routing is richer; our `default_model` hint is a subset. Merge hint into upstream routing config on startup path. |
| `7672ca9044` | feat(skills): add native tool handler for SKILL.toml-based skills | Upstream SkillForge + `8a4da141` | Upstream injects skill tools; our SKILL.toml native handler is a different contract. Align with upstream skill contract during port. |
| `6716391502` | feat: harden web access policy and add flexible web search/runtime config | `554ee9ce` `feat(tools): add proxy support to web_search_tool` | Both sides evolved web access controls. Our `web_access_config.rs` + `web_search_config.rs` differs from upstream's proxy-based approach. Merge the allowlist/CIDR model into upstream's security layer. |
| `b4df1dc30d` | feat(tools): add web_fetch provider dispatch and shared URL validation | `71d3730a` `feat(web_fetch): add allowed_private_hosts config` + upstream `web_fetch.rs` | Upstream has web_fetch with private-host config; our version adds provider dispatch. Merge dispatch logic into upstream web_fetch. |
| `479b6da4ce` | feat(cost): wire provider token usage to cost tracking (#2111) | `e407415a` `fix(cost,cron,channel): capture model cost` | Both sides wire cost tracking; upstream may be a superset. Verify diff before porting — may resolve to ADOPT. |
| `dcd712d825` + `6a228944ae` | feat(tools): add Feishu document operation tool with 13 actions | `2d73133d` `feat: feishu channel support transport media` | Upstream has Feishu media transport but not a 13-action document operation tool. Our `feishu_doc.rs` is unique in its actions; however upstream's feishu integration has evolved significantly. Reconcile by porting our action set into upstream's feishu crate structure. |

---

### DROP

Commits that are pure churn, CI-only work that belongs in upstream's CI, duplicates, or superseded refactors not worth carrying.

| SHA | Subject | Reason |
|---|---|---|
| `46d087eb8f` | refactor(update): remove unused ErrorKind import | Trivial lint cleanup; upstream is cleaner. No functional content. |
| `13469f0839` | refactor(telegram): remove redundant else in startup probe | Two-line refactor; upstream telegram already cleaner. No functional content. |
| `4756d70d95` | feat(workspace): scaffold M4-5 crate shells and CI package lanes | Workspace scaffolding for our internal milestone structure (M4-5). Upstream has its own workspace layout. Our crate shells are project management artifacts. |
| `c53e023b81` | feat(ci): add nightly profile retries and trend snapshot evidence | CI configuration for our self-hosted runners; not relevant to upstream-based CI. |
| `d9a81409fb` | feat(ci): formalize canary cohorts and observability policy | Our canary deployment CI policy; not applicable to upstream CI. |
| `4e7c3dcc13` | feat(ci): enforce docs deploy promotion and rollback contract | Our CI docs deployment; not relevant. |
| `83d5421368` | feat(ci): add release/canary/nightly automation and governance guards | Our release pipeline; use upstream release CI. |
| `864684a5d0` | feat(ci): add MUSL static binaries for release artifacts | Our release CI; upstream has its own release matrix. |
| `fcc3d0e93a` | feat(release): automate supply-chain release notes preface | Our release process automation; not upstream. |
| `629253f63e` | feat(release): enforce artifact contract guard | Same — our release governance. |
| `5e91f074a8` | feat(ci): add release trigger authorization guard | Our CI governance. |
| `c2fd20cf25` | feat(ci): harden prerelease stage matrix and transition audit | Our CI. |
| `d579fb9c3c` | feat(ci): bridge canary abort to rollback guard dispatch | Our deployment CI. |
| `211bff082b` | perf(ci): optimize CI/CD pipeline critical path | Our CI runner optimization. |
| `8f91f956fd` | feat(ci): complete security audit governance and resilient CI control lanes | Our CI. |
| `30d8a8b33b` | feat(ci): add unsafe debt audit report script | Our CI tooling. |
| `523fecac0f` | refactor(agent): satisfy strict lint delta for loop split | Lint-only cleanup paired with `1b12f60e05`; the functional work is in the other commit. |
| `f218a35ee5` + `011b379bec` | feat(unsafe-debt): integrate/deepen crate-root guard enforcement | Internal unsafe-debt audit work for our CI governance; not functional product code. |

---

### DEFER (time-boxed out)

Commits that need additional investigation to classify correctly. Most require reading the actual diff rather than just the subject line.

| SHA | Subject | What needs investigating |
|---|---|---|
| `404c43bbe3` | Feature/multitenant deployment enhancements (#2380) | Large PR — need to see which parts are Aura-specific deployment vs generic. Check if upstream absorbed any of this. |
| `f7167ea485` | feat(agent): add normalized stop reasons and max-token continuation | Need to verify upstream stop_reason handling in 0.7.3; may already exist. |
| `6d25a060c1` | feat(skills): add trusted domain policy and transparent preloads | Unclear if this overlaps with `3ea99a76` (browser delegation) or is a separate trust layer. |
| `be0f52fce7` | feat(agent): add end-to-end team orchestration bundle | Large feature; need to diff against `21bd251d` (shared iteration budget upstream) and confirm delta. |
| `1431e9e864` | feat(memory): add time-decay scoring with Core evergreen exemption | Upstream has `a9ffd389` "restore time-decay scoring" — need to check if upstream restoration makes ours redundant. |
| `20d4e1599a` | feat(skills): add trusted symlink roots for workspace skills | Unclear overlap with upstream skill path traversal fix (`641a5bf9`). |
| `1ecace23a7` | feat(update): add install-aware guidance and safer self-update | Partially overlaps ADOPT `390373dbcb`; need to check if guidance component is unique. |
| `2d5c0142d2` | feat(auth): improve OAuth UX for server environments | Need to check if upstream's server-flow improvements (`34baae91`) cover this. |
| `4ce4ec5f34` | feat(security): allow read-only git config operations | Need to verify this isn't already in upstream's `git_operations.rs` security policy. |
| `408616b34e` | feat(agent): expose hooks parameter in public run() entry point | Need to check if upstream run() API already has hooks parameter after workspace split. |
| `28b9d81464` | security: add /mnt to default forbidden_paths | Need to check if `85f9e6a8` (path-guard wrappers) covers /mnt. |
| `579f0f3d9a` | feat(channels): add comprehensive ACP channel tests and fix implementation bugs | Upstream has ACP (`5c81d4e4`, `1bfc1537`) — need to check if our bug fixes are still needed. |
| `11b08d2184` | feat(web): add data-driven config form editor with category navigation | Gateway web UI feature; check if upstream dashboard evolved to cover this. |
| `9784e3bfc1` | feat(channel): add github native channel MVP | No upstream github channel in channels tree; but need to check webhook gateway approach upstream used. |
| `f6278373cb` | feat: add cursor headless cli support (#2195) | Need to check if our `src/providers/cursor.rs` is still relevant post-upstream workspace split. |
| `20ed60d2a0` | feat(config): add show/get/set subcommands for runtime config inspection | Upstream has `fb32a89f` (Vec<String> get/set/list) — check if our `show` subcommand is superseded. |
| `b01462d7a9` | feat(gemini): support multimodal inlineData in user messages | Need to check upstream's `9e8a4782` Gemini vision support for overlap. |
| `561c4765e1` | feat(providers): add responses-mode chat-completions fallback (#2417) | Upstream has responses fallback (`f17c6dce`, `03d345b5`); need to verify our version adds anything. |
| `237845f490` | feat(cli): include git short sha in version output | Quick check: does upstream version output include SHA? |
| `34852919da` | feat(onboard): support identity backend selection and AIEOS scaffolding | AIEOS scaffolding may be Aura-specific; need to isolate identity backend part. |
| `77c6aba24c` | feat(provider): add qwen-coding-plan endpoint alias | Check if upstream `f8a57e1e` (Alibaba Coding Plan) covers this. |
| `a258741e2f` | feat(security): enable otp by default in quick setup | Duplicate of `5ecea422c7` in list — check which is canonical. |
| `5ecea422c7` | feat(security): enable otp by default in quick setup | Pair with above — need to determine which is the keeper commit. |
| `96d941f83a` + `e92a976226` | feat(discord): forward inbound image attachments as markers | Two near-duplicate commits; check upstream `11153b6a` (reaction support) for image attachment handling. |
| `b2462585b7` | feat(android): add Android client foundation | Large; check if upstream Android target (`aa45c30e`) covers this or if ours is divergent UniFFI bridge. |
| `dd94cac1bd` | feat(android): Phase 2 - UniFFI bridge and settings UI | Depends on b2462585b7 — defer together. |
| `da899a3046` | feat(android): Phase 3 - WorkManager, tiles, battery optimization | Same android chain. |
| `8a1dea306e` | feat(android): Phase 4 - Widget, accessibility, one-liner installers | Same android chain. |
| `c1f255af96` | perf(android): aggressive binary size optimization | Same android chain — likely DROP if we're not shipping an Android app. |
| `5d2472bd56` | feat(android): add strict self-check mode with warning gates | Same android chain. |
| `664dcdcb82` | feat(android): standardize self-check error codes and offline diagnostics | Same android chain. |
| `48cba9e076` | feat(android): add structured error codes and stdout JSON mode | Same android chain. |
| `88f7d842e5` | feat(android): add JSON self-check report and regression tests | Same android chain. |
| `424f67d948` | feat(android): support offline log diagnosis and tests | Same android chain. |
| `3b8fbcaa38` | feat(android): auto-diagnose cargo check toolchain failures | Same android chain. |
| `e5aacec1a5` | feat(android): add mode-aware source-build self-check | Same android chain. Android chain decision: if Aura doesn't ship Android, DROP all; if it does, KEEP all. |
| `b228800e9e` | feat(web): add zh-CN locale support | Need to check if upstream `0d41670c` (i18n for 31 README languages) covers web UI locale. |
| `18780b27fe` | feat: add OpenAI-compatible /v1/chat/completions and /v1/models endpoints | Need to diff against our AURA_GLUE gateway compat; may overlap with `dde8b82ea0`. |
| `d9b3d6f3e5` + `9fbab15222` + `e07c4d29cd` + `44bcb4cd6b` | feat(site): docs hub/reader commits (4 commits) | These are website/docs commits — DROP if we run our own docs, KEEP if zeroclaw site is shared. |
| `03bf3f105d` | feat(integrations): enhance integrations settings UX and provider metadata | Gateway web UI; check overlap with upstream dashboard. |
| `47ad3d010b` | feat(integrations): add list and search subcommands | CLI integrations list/search — check upstream `3d91c409` (simplify CLI). |
| `1a0372709d` | feat(whatsapp): support heartbeat and cron delivery for whatsapp_web | Upstream heartbeat evolved; check `c86a0673` (two-phase heartbeat). |
| `667c7a4c2f` | hardening(deps): govern matrix indexeddb derivative advisory | May be a dep advisory response — check if upstream fixed the advisory. |
| `b238e8fd5e` (economic config) | Already in KEEP-OURS | Listed twice in input; ignore. |
| `8f263cd336` | feat(agent): add CLI parameters for runtime config overrides | Check upstream `fb32a89f` (config get/set/list) for overlap. |
| `1b12f60e05` | refactor(agent): split loop loop_ concerns into focused submodules | Structural refactor — check if upstream `98eb378c` (extract cost/history/tool_execution) already splits this. |
| `0e14c199af` | refactor(tools): deduplicate IpcDb initialization and simplify inbox | Part of IPC tool chain; keep if IPC tools (KEEP-OURS) are ported. |
| `b4df1dc30d` | feat(tools): add web_fetch provider dispatch | Already listed in MERGE; defer the provider dispatch portion. |
| `666f1a7d10` | feat(provider): add responses websocket transport fallback | Need to check upstream `1da35dbc` (Z.AI streaming) and `f17c6dce` (responses fallback); may be ADOPT. |
| `f218a35ee5` | feat(unsafe-debt): integrate policy-driven audit coverage | Already in DROP; confirm it is CI-only and has no runtime code. |

---

## M3.9 pre-port list

The KEEP-OURS commits NOT already covered by M3.2–M3.8. These feed the M3.9 porting task (Row 4 of the plan).

**Memory subsystem** (depends on M3.2 post-turn memory):
- `3fb11acade` feat(memory): boost core memories during context retrieval → `src/memory/`
- `2052c720cc` feat(memory): flush durable facts before compaction → `src/memory/`
- `0253752bc9` feat(memory): add observation memory tool → `src/tools/memory_observe.rs`
- `84b43ba4b2` feat(memory): add reindex command → `src/memory/cli.rs`
- `f4d06a3a73` feat(memory): add optional cortex-mem backend profile → `src/memory/`
- `498fca9d08` feat(memory): add sqlite+qdrant hybrid backend → `src/memory/`
- `0129b5da06` feat(onboard): add hybrid sqlite+qdrant memory option → `src/onboard/`
- `9ecb8dffa6` feat(memory): add sqlite_journal_mode config → `src/config/schema.rs`

**Security subsystem** (depends on M3.3 canary, M3.4 approval):
- `09d32dcd79` feat(security): context-aware command allow rules → `src/security/`
- `a029c720a6` feat(security): safety heartbeat reinjection → `src/security/`
- `51ad52d0e8` security: harden sensitive I/O and outbound leak controls → `src/security/`
- `bfe3e4295d` feat(security): opt-in perplexity adversarial suffix filter → `src/security/`
- `970ef57f21` feat(security): aho-corasick and entropy leak heuristics → `src/security/`
- `d63a6a8ceb` feat(security): unify URL validation with CIDR/domain allowlist → `src/tools/url_validation.rs`
- `cfe1e578bf` feat(security): syscall anomaly detection → `src/security/`
- `268b01fcf0` hardening(security): sanitize channel error bodies → `src/security/`
- `346f58a6a1` hardening: strengthen tool policy enforcement → `src/security/`
- `a09f146145` / `7aee6d9dc7` feat(security): role-policy and OTP challenge → `src/security/`

**Agent / loop subsystem**:
- `0b5665ad9b` feat(agent): adaptive load balancing → `src/agent/`
- `49384b1678` feat(agent): intelligent team/subagent orchestration → `src/agent/`
- `a1d51b6454` feat(agent): ProgressTracker → `src/agent/`
- `bfacba20cb` feat(config): ProgressMode enum → `src/config/schema.rs`
- `cc9ff1820b` feat(autonomy): exclude process default for non-cli → `src/channels/`
- `6662601a6c` feat(agent): result-aware loop detection → `src/agent/`
- `817f783881` feat(agent): inject shell allowlist into system prompt → `src/agent/`
- `8f263cd336` feat(agent): CLI parameters for runtime config overrides → `src/agent/` (DEFER — check first)

**Tools**:
- `f3c82cb13a` xlsx_read → `src/tools/xlsx_read.rs`
- `df6f7455e7` docx_read → `src/tools/docx_read.rs`
- `cb1cd14cbb` pptx_read → `src/tools/pptx_read.rs`
- `fbb3c6aee0` apply_patch → `src/tools/apply_patch.rs`
- `d943f9c28c` bg_run → `src/tools/bg_run.rs`
- `da62bd172f` user_agent config → `src/tools/web_access_config.rs`
- `067eb8a188` sub-agent orchestration → `src/tools/subagent_*.rs`
- `ed67184c7a` IPC tools → `src/tools/agents_ipc.rs`
- `d6d32400fa` task_plan → `src/tools/task_plan.rs`
- `56ffcd4477` background process management → `src/tools/process.rs`
- `d5fe47acff` auth_profile + quota wiring → `src/tools/auth_profile.rs`
- `f0a5bbdb1b` http_request env credential profiles → `src/tools/http_request.rs`

**Providers**:
- `8c0be20422` quota_metadata on ChatResponse → `src/providers/`
- `247d89e39e` quota monitoring system → `src/tools/quota_tools.rs` + `src/providers/quota_*.rs`
- `d2b0338afd` circuit breaker / provider health → `src/providers/health.rs`
- `3a38c80c05` model_support_vision override → `src/config/schema.rs`

**Channels**:
- `3f6e192b14` Discord approval buttons → `src/channels/discord.rs`
- `9fb0e30dac` hot-reload runtime tool-loop defaults → `src/channels/`
- `4d195be713` BlueBubbles iMessage → `src/channels/bluebubbles.rs`
- `cc0bc49b2f` napcat QQ → `src/channels/napcat.rs`
- `5aac1af065` onebot aliases → `src/channels/napcat.rs`
- `54dd7a4a9b` QQ webhook receive mode → `src/channels/`
- `60b73b6cd3` Slack socket mode fallback → `src/channels/slack.rs`
- `61d538b6d6` Slack multi-channel IDs → `src/channels/slack.rs`
- `87fa327e0d` Telegram ack_enabled → `src/channels/telegram.rs`
- `16961bab84` hide internal tool progress → `src/channels/`
- `a9e8526d67` unified group-reply policy → `src/channels/`
- `883f92409e` query classification routing + logging → `src/channels/`
- `2ecfa0d269` channel tool boundaries + websocket auth → `src/channels/`
- `cc8aac5918` Lark rich-text, WhatsApp QR, draft config → `src/channels/`

**Cost / Economic**:
- `4043056332` cost preflight budget policy → `src/cost/`
- `479b6da4ce` cost tracking wire (MERGE first) → `src/cost/`
- `a2f1f09364` TaskClassifier → `src/economic/`
- `b238e8fd5e` economic agent config schema → `src/config/schema.rs`
- `9798b34f8d` ClawWork module → `src/economic/`

**Coordination / Goals**:
- `856afe8780` agent coordination message bus → `src/coordination/`
- `6064890415` goals engine + heartbeat delivery → `src/goals/`

**Approval** (depends on M3.4):
- `1ad2d71c9b` one-time all-tools non-cli approval → `src/approval/`
- `1fcf2df28b` harden non-CLI approval governance → `src/approval/`
- `91d8abf723` WATI webhook auth failure metric → `src/observability/`

---

## Notes for M3.9 execution

1. **Android chain** (10+ commits): Aura does not ship an Android client. All Android commits in DEFER should resolve to DROP unless a decision is made to adopt the Android client. Decision needed from product.

2. **Site/docs commits** (`d9b3d6f3e5`, `9fbab15222`, `e07c4d29cd`, `44bcb4cd6b`): These are zeroclaw.io website commits. DROP for Aura; the website is upstream's concern.

3. **Duplicate OTP commits** (`5ecea422c7` and `a258741e2f`): These appear to be the same feature committed twice. Keep only the later/cleaner one. Check `git show` to confirm.

4. **WASM/plugin system**: Our plugin system (KEEP-OURS `362a81a3e5`, `467fea87c6`, `ade0e91898`) predates upstream's Extism-based system. The upstream Extism implementation is likely richer — the ADOPT verdicts above cover the Extism runtime itself, but our `HookRunner` factory (`467fea87c6`) may be needed for Aura's hook lifecycle regardless. Review during port.

5. **Cost tracking convergence** (`479b6da4ce` MERGE): Upstream `e407415a` landed a comprehensive cost capture. Compare diffs before porting to avoid regression.

6. **Feishu doc tool** (`dcd712d825` + `6a228944ae` — two commits for same feature): Both are listed; the second (`6a228944ae`) appears to be a re-application. Take only the canonical version during port.
