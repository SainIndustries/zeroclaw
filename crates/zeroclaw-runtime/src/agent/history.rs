use crate::agent::history_pruner::remove_orphaned_tool_messages;
use crate::util::truncate_with_ellipsis;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::path::Path;
use zeroclaw_memory::{Memory, MemoryCategory};
use zeroclaw_providers::{ChatMessage, Provider};
use zeroclaw_providers::scrub_secret_patterns;

/// Default trigger for auto-compaction when non-system message count exceeds this threshold.
/// Prefer passing the config-driven value via `run_tool_call_loop`; this constant is only
/// used when callers omit the parameter.
pub const DEFAULT_MAX_HISTORY_MESSAGES: usize = 50;

/// Find the largest byte index `<= i` that is a valid char boundary.
/// MSRV-compatible replacement for `str::floor_char_boundary` (stable in 1.91).
pub fn floor_char_boundary(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    let mut pos = i;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Truncate a tool result to `max_chars`, keeping head (2/3) + tail (1/3)
/// with a marker in the middle. Returns input unchanged if within limit or
/// `max_chars == 0` (disabled).
pub fn truncate_tool_result(output: &str, max_chars: usize) -> String {
    if max_chars == 0 || output.len() <= max_chars {
        return output.to_string();
    }
    let head_len = max_chars * 2 / 3;
    let tail_len = max_chars.saturating_sub(head_len);
    let head_end = floor_char_boundary(output, head_len);
    // ceil_char_boundary: find smallest byte index >= i on a char boundary
    let tail_start_raw = output.len().saturating_sub(tail_len);
    let tail_start = if tail_start_raw >= output.len() {
        output.len()
    } else {
        let mut pos = tail_start_raw;
        while pos < output.len() && !output.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    };
    // Guard against overlap when max_chars is very small
    if head_end >= tail_start {
        return output[..floor_char_boundary(output, max_chars)].to_string();
    }
    let truncated_chars = tail_start - head_end;
    format!(
        "{}\n\n[... {} characters truncated ...]\n\n{}",
        &output[..head_end],
        truncated_chars,
        &output[tail_start..]
    )
}

/// Truncate a tool message's content, preserving JSON structure when the
/// message stores `tool_call_id` alongside `content` (native tool-call
/// format). Without this, `truncate_tool_result` destroys the JSON envelope
/// and downstream providers receive a `null` `call_id` (#5425).
pub fn truncate_tool_message(msg_content: &str, max_chars: usize) -> String {
    if max_chars == 0 || msg_content.len() <= max_chars {
        return msg_content.to_string();
    }
    if let Ok(mut obj) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(msg_content)
        && obj.contains_key("tool_call_id")
        && let Some(serde_json::Value::String(inner)) = obj.get("content")
    {
        let truncated = truncate_tool_result(inner, max_chars);
        obj.insert("content".to_string(), serde_json::Value::String(truncated));
        return serde_json::to_string(&obj).unwrap_or_else(|_| msg_content.to_string());
    }
    truncate_tool_result(msg_content, max_chars)
}

/// Aggressively trim old tool result messages in history to recover from
/// context overflow. Keeps the last `protect_last_n` messages untouched.
/// Returns total characters saved.
pub fn fast_trim_tool_results(
    history: &mut [zeroclaw_providers::ChatMessage],
    protect_last_n: usize,
) -> usize {
    let trim_to = 2000;
    let mut saved = 0;
    let cutoff = history.len().saturating_sub(protect_last_n);
    for msg in &mut history[..cutoff] {
        if msg.role == "tool" && msg.content.len() > trim_to {
            let original_len = msg.content.len();
            msg.content = truncate_tool_message(&msg.content, trim_to);
            saved += original_len - msg.content.len();
        }
    }
    saved
}

/// Emergency: drop oldest non-system, non-recent messages from history.
/// Tool groups (assistant + consecutive tool messages) are dropped
/// atomically to preserve tool_use/tool_result pairing. See #4810.
/// Returns number of messages dropped.
pub fn emergency_history_trim(
    history: &mut Vec<zeroclaw_providers::ChatMessage>,
    keep_recent: usize,
) -> usize {
    let mut dropped = 0;
    let target_drop = history.len() / 3;
    let mut i = 0;
    while dropped < target_drop && i < history.len().saturating_sub(keep_recent) {
        if history[i].role == "system" {
            i += 1;
        } else if history[i].role == "assistant" {
            // Count following tool messages — drop as atomic group
            let mut tool_count = 0;
            while i + 1 + tool_count < history.len().saturating_sub(keep_recent)
                && history[i + 1 + tool_count].role == "tool"
            {
                tool_count += 1;
            }
            for _ in 0..=tool_count {
                history.remove(i);
                dropped += 1;
            }
        } else {
            history.remove(i);
            dropped += 1;
        }
    }
    dropped += remove_orphaned_tool_messages(history);
    dropped
}

/// Estimate token count for a message history using ~4 chars/token heuristic.
/// Includes a small overhead per message for role/framing tokens.
pub fn estimate_history_tokens(history: &[ChatMessage]) -> usize {
    history
        .iter()
        .map(|m| {
            // ~4 chars per token + ~4 framing tokens per message (role, delimiters)
            m.content.len().div_ceil(4) + 4
        })
        .sum()
}

/// Trim conversation history to prevent unbounded growth.
/// Preserves the system prompt (first message if role=system) and the most recent messages.
pub fn trim_history(history: &mut Vec<ChatMessage>, max_history: usize) {
    // Nothing to trim if within limit
    let has_system = history.first().is_some_and(|m| m.role == "system");
    let non_system_count = if has_system {
        history.len() - 1
    } else {
        history.len()
    };

    if non_system_count <= max_history {
        return;
    }

    let start = if has_system { 1 } else { 0 };
    let to_remove = non_system_count - max_history;
    history.drain(start..start + to_remove);
    remove_orphaned_tool_messages(history);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveSessionState {
    pub version: u32,
    pub history: Vec<ChatMessage>,
}

impl InteractiveSessionState {
    fn from_history(history: &[ChatMessage]) -> Self {
        Self {
            version: 1,
            history: history.to_vec(),
        }
    }
}

pub fn load_interactive_session_history(
    path: &Path,
    system_prompt: &str,
) -> Result<Vec<ChatMessage>> {
    if !path.exists() {
        return Ok(vec![ChatMessage::system(system_prompt)]);
    }

    let raw = std::fs::read_to_string(path)?;
    let mut state: InteractiveSessionState = serde_json::from_str(&raw)?;
    if state.history.is_empty() {
        state.history.push(ChatMessage::system(system_prompt));
    } else if state.history.first().map(|msg| msg.role.as_str()) != Some("system") {
        state.history.insert(0, ChatMessage::system(system_prompt));
    }

    Ok(state.history)
}

pub fn save_interactive_session_history(path: &Path, history: &[ChatMessage]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let payload = serde_json::to_string_pretty(&InteractiveSessionState::from_history(history))?;
    std::fs::write(path, payload)?;
    Ok(())
}

// ── Post-turn durable fact extraction ──────────────────────────────────────

/// Safety cap for durable facts extracted during pre-compaction flush.
pub(crate) const COMPACTION_MAX_FLUSH_FACTS: usize = 8;

/// Number of conversation turns between automatic fact extractions.
const EXTRACT_TURN_INTERVAL: usize = 5;

/// Minimum combined character count (user + assistant) to trigger extraction.
const EXTRACT_MIN_CHARS: usize = 200;

/// Safety cap for fact-extraction transcript sent to the LLM.
const EXTRACT_MAX_SOURCE_CHARS: usize = 12_000;

/// Maximum characters for the "already known facts" section injected into
/// the extraction prompt.  Keeps token cost bounded when recall returns
/// long entries.
const KNOWN_SECTION_MAX_CHARS: usize = 2_000;

/// Maximum length (in chars) for a normalized fact key.
const FACT_KEY_MAX_LEN: usize = 64;

/// Substrings that indicate a fact is purely a secret shell after redaction.
const SECRET_SHELL_PATTERNS: &[&str] = &[
    "api key",
    "api_key",
    "token",
    "password",
    "secret",
    "credential",
    "access key",
    "access_key",
    "private key",
    "private_key",
];

/// Accumulates conversation turns for periodic fact extraction.
///
/// Decoupled from `history` so tool/summary messages do not affect
/// the extraction window.
pub(crate) struct TurnBuffer {
    turns: Vec<(String, String)>,
    total_chars: usize,
    last_extract_succeeded: bool,
}

/// Outcome of a single extraction attempt.
pub(crate) struct ExtractionResult {
    /// Number of facts successfully stored to Core memory.
    pub stored: usize,
    /// `true` when the LLM confirmed there are no new facts (or all parsed
    /// facts were intentionally skipped). `false` on LLM/store failures.
    pub no_facts: bool,
}

impl TurnBuffer {
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            total_chars: 0,
            last_extract_succeeded: true,
        }
    }

    /// Record a completed conversation turn.
    pub fn push(&mut self, user_msg: &str, assistant_resp: &str) {
        self.total_chars += user_msg.chars().count() + assistant_resp.chars().count();
        self.turns
            .push((user_msg.to_string(), assistant_resp.to_string()));
    }

    /// Whether the buffer has accumulated enough turns and content to
    /// justify an extraction call.
    pub fn should_extract(&self) -> bool {
        self.turns.len() >= EXTRACT_TURN_INTERVAL && self.total_chars >= EXTRACT_MIN_CHARS
    }

    /// Drain all buffered turns and return them for extraction.
    /// Resets character counter; `last_extract_succeeded` is cleared
    /// until the caller confirms success via [`mark_extract_success`].
    pub fn drain_for_extraction(&mut self) -> Vec<(String, String)> {
        self.total_chars = 0;
        self.last_extract_succeeded = false;
        std::mem::take(&mut self.turns)
    }

    /// Mark the most recent extraction as successful.
    pub fn mark_extract_success(&mut self) {
        self.last_extract_succeeded = true;
    }

    /// Whether there are buffered turns that have not been extracted.
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Whether compaction should fall back to its own `flush_durable_facts`.
    /// This returns `true` when un-extracted turns remain **or** the last
    /// extraction failed (so durable facts may have been lost).
    pub fn needs_compaction_fallback(&self) -> bool {
        !self.turns.is_empty() || !self.last_extract_succeeded
    }
}

/// Extract durable facts from recent conversation turns and store them
/// as `Core` memories.
///
/// Best-effort: failures are logged but never block the caller.
///
/// This is the unified extraction entry-point used by all agent entry
/// points (single-message, interactive, channel, `Agent` struct).
pub(crate) async fn extract_facts_from_turns(
    provider: &dyn Provider,
    model: &str,
    turns: &[(String, String)],
    memory: &dyn Memory,
    session_id: Option<&str>,
) -> ExtractionResult {
    let empty = ExtractionResult {
        stored: 0,
        no_facts: true,
    };

    if turns.is_empty() {
        return empty;
    }

    // Build transcript from buffered turns.
    let mut transcript = String::new();
    for (user, assistant) in turns {
        let _ = writeln!(transcript, "USER: {}", user.trim());
        let _ = writeln!(transcript, "ASSISTANT: {}", assistant.trim());
        transcript.push('\n');
    }

    let total_chars: usize = turns
        .iter()
        .map(|(u, a)| u.chars().count() + a.chars().count())
        .sum();
    if total_chars < EXTRACT_MIN_CHARS {
        return empty;
    }

    // Truncate to avoid oversized LLM prompts with very long messages.
    if transcript.chars().count() > EXTRACT_MAX_SOURCE_CHARS {
        transcript = truncate_with_ellipsis(&transcript, EXTRACT_MAX_SOURCE_CHARS);
    }

    // Recall existing memories for dedup context.
    let existing = memory
        .recall(&transcript, 10, session_id, None, None)
        .await
        .unwrap_or_default();

    let mut known_section = String::new();
    if !existing.is_empty() {
        known_section.push_str(
            "\nYou already know these facts (do NOT repeat them; \
             use the SAME key if a fact needs updating):\n",
        );
        for entry in &existing {
            let line = format!("- {}: {}\n", entry.key, entry.content);
            if known_section.chars().count() + line.chars().count() > KNOWN_SECTION_MAX_CHARS {
                known_section.push_str("- ... (truncated)\n");
                break;
            }
            known_section.push_str(&line);
        }
    }

    let system_prompt = format!(
        "You extract durable facts from a conversation. \
         Output ONLY facts worth remembering long-term \u{2014} user preferences, project decisions, \
         technical constraints, commitments, or important discoveries.\n\
         \n\
         NEVER extract secrets, API keys, tokens, passwords, credentials, \
         or any sensitive authentication data. If the conversation contains \
         such data, skip it entirely.\n\
         {known_section}\n\
         Output one fact per line, prefixed with a short key in brackets.\n\
         Example:\n\
         [preferred_language] User prefers Rust over Go\n\
         [db_choice] Project uses PostgreSQL 16\n\
         If there are no new durable facts, output exactly: NONE"
    );

    let user_prompt = format!(
        "Extract durable facts from this conversation (max {} facts):\n\n{}",
        COMPACTION_MAX_FLUSH_FACTS, transcript
    );

    let response = match provider
        .chat_with_system(Some(&system_prompt), &user_prompt, model, 0.2)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Post-turn fact extraction failed: {e}");
            return ExtractionResult {
                stored: 0,
                no_facts: false,
            };
        }
    };

    if response.trim().eq_ignore_ascii_case("NONE") {
        return empty;
    }
    if response.trim().is_empty() {
        // Provider returned empty — treat as failure so compaction
        // fallback remains active.
        return ExtractionResult {
            stored: 0,
            no_facts: false,
        };
    }

    let mut stored = 0usize;
    let mut parsed = 0usize;
    let mut store_failures = 0usize;
    for line in response.lines() {
        if stored >= COMPACTION_MAX_FLUSH_FACTS {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, content)) = parse_fact_line(line) {
            parsed += 1;
            // Scrub secrets from extracted content.
            let clean = scrub_secret_patterns(content);
            if should_skip_redacted_fact(&clean, content) {
                tracing::info!("Skipped fact '{key}': only secret shell remains after redaction");
                continue;
            }
            let norm_key = normalize_fact_key(key);
            if norm_key.is_empty() {
                continue;
            }
            let prefixed_key = format!("auto_{norm_key}");
            if let Err(e) = memory
                .store(&prefixed_key, &clean, MemoryCategory::Core, session_id)
                .await
            {
                tracing::warn!("Failed to store extracted fact '{prefixed_key}': {e}");
                store_failures += 1;
            } else {
                stored += 1;
            }
        }
    }
    if stored > 0 {
        tracing::info!("Post-turn extraction: stored {stored} durable fact(s) to Core memory");
    }

    // When parsed == 0 (unparseable output) or store_failures > 0 (backend
    // errors), treat as failure so compaction fallback remains active.
    ExtractionResult {
        stored,
        no_facts: parsed > 0 && stored == 0 && store_failures == 0,
    }
}

/// Extract durable facts from a conversation transcript and store them as
/// `Core` memories. Called before compaction discards old messages.
///
/// Best-effort: failures are logged but never block compaction.
/// Returns `true` when facts were stored **or** the LLM confirmed
/// there are none (`NONE` response). Returns `false` on LLM/store
/// failures so the caller can avoid marking extraction as successful.
pub(crate) async fn flush_durable_facts(
    provider: &dyn Provider,
    model: &str,
    transcript: &str,
    memory: &dyn Memory,
    session_id: Option<&str>,
) -> bool {
    const FLUSH_SYSTEM: &str = "\
You extract durable facts from a conversation that is about to be compacted. \
Output ONLY facts worth remembering long-term — user preferences, project decisions, \
technical constraints, commitments, or important discoveries.\n\
\n\
NEVER extract secrets, API keys, tokens, passwords, credentials, \
or any sensitive authentication data. If the conversation contains \
such data, skip it entirely.\n\
\n\
Output one fact per line, prefixed with a short key in brackets. \
Example:\n\
[preferred_language] User prefers Rust over Go\n\
[db_choice] Project uses PostgreSQL 16\n\
If there are no durable facts, output exactly: NONE";

    let flush_user = format!(
        "Extract durable facts from this conversation (max 8 facts):\n\n{}",
        transcript
    );

    let response = match provider
        .chat_with_system(Some(FLUSH_SYSTEM), &flush_user, model, 0.2)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Pre-compaction memory flush failed: {e}");
            return false;
        }
    };

    if response.trim().eq_ignore_ascii_case("NONE") {
        return true; // genuinely no facts
    }
    if response.trim().is_empty() {
        return false; // provider returned empty — treat as failure
    }

    let mut stored = 0usize;
    let mut parsed = 0usize;
    let mut store_failures = 0usize;
    for line in response.lines() {
        if stored >= COMPACTION_MAX_FLUSH_FACTS {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Parse "[key] content" format
        if let Some((key, content)) = parse_fact_line(line) {
            parsed += 1;
            // Scrub secrets from extracted content.
            let clean = scrub_secret_patterns(content);
            if should_skip_redacted_fact(&clean, content) {
                tracing::info!(
                    "Skipped compaction fact '{key}': only secret shell remains after redaction"
                );
                continue;
            }
            let norm_key = normalize_fact_key(key);
            if norm_key.is_empty() {
                continue;
            }
            let prefixed_key = format!("auto_{norm_key}");
            if let Err(e) = memory
                .store(&prefixed_key, &clean, MemoryCategory::Core, session_id)
                .await
            {
                tracing::warn!("Failed to store compaction fact '{prefixed_key}': {e}");
                store_failures += 1;
            } else {
                stored += 1;
            }
        }
    }
    if stored > 0 {
        tracing::info!("Pre-compaction flush: stored {stored} durable fact(s) to Core memory");
    }
    // Success when at least one fact was parsed and no store failures
    // occurred, OR all parsed facts were intentionally skipped.
    // Unparseable output (parsed == 0) is treated as failure.
    parsed > 0 && store_failures == 0
}

/// Parse a `[key] content` line from the fact extraction output.
pub(crate) fn parse_fact_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_start_matches(|c: char| c == '-' || c.is_whitespace());
    let rest = line.strip_prefix('[')?;
    let close = rest.find(']')?;
    let key = rest[..close].trim();
    let content = rest[close + 1..].trim();
    if key.is_empty() || content.is_empty() {
        return None;
    }
    Some((key, content))
}

/// Normalize a fact key to a consistent `snake_case` form with length cap.
///
/// - Replaces whitespace/hyphens with underscores
/// - Lowercases
/// - Strips non-alphanumeric (except `_`)
/// - Collapses repeated underscores
/// - Truncates to [`FACT_KEY_MAX_LEN`]
fn normalize_fact_key(raw: &str) -> String {
    let mut key: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    // Collapse repeated underscores.
    while key.contains("__") {
        key = key.replace("__", "_");
    }
    let key = key.trim_matches('_');
    if key.chars().count() > FACT_KEY_MAX_LEN {
        key.chars().take(FACT_KEY_MAX_LEN).collect()
    } else {
        key.to_string()
    }
}

/// Decide whether a redacted fact should be skipped.
///
/// A fact is skipped when scrubbing removed secrets and the remaining
/// text is empty or consists solely of generic secret-type labels
/// (e.g. "api key", "token").
fn should_skip_redacted_fact(clean: &str, original: &str) -> bool {
    // No redaction happened — always keep.
    if clean == original {
        return false;
    }
    let remainder = clean.replace("[REDACTED]", "").trim().to_lowercase();
    let remainder = remainder.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace());
    if remainder.is_empty() {
        return true;
    }
    SECRET_SHELL_PATTERNS.contains(&remainder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use zeroclaw_memory::{MemoryCategory, MemoryEntry};
    use zeroclaw_providers::{ChatRequest, ChatResponse, Provider};

    // ── Shared test helpers ────────────────────────────────────────

    struct StaticSummaryProvider;

    #[async_trait]
    impl Provider for StaticSummaryProvider {
        async fn chat_with_system(
            &self,
            _system_prompt: Option<&str>,
            _message: &str,
            _model: &str,
            _temperature: f64,
        ) -> anyhow::Result<String> {
            Ok("- summarized context".to_string())
        }

        async fn chat(
            &self,
            _request: ChatRequest<'_>,
            _model: &str,
            _temperature: f64,
        ) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse {
                text: Some("- summarized context".to_string()),
                tool_calls: Vec::new(),
                usage: None,
                reasoning_content: None,
            })
        }
    }

    struct NoopMem;

    #[async_trait]
    impl Memory for NoopMem {
        async fn store(&self, _k: &str, _c: &str, _cat: MemoryCategory, _s: Option<&str>) -> anyhow::Result<()> { Ok(()) }
        async fn recall(&self, _q: &str, _l: usize, _s: Option<&str>, _since: Option<&str>, _until: Option<&str>) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
        async fn get(&self, _k: &str) -> anyhow::Result<Option<MemoryEntry>> { Ok(None) }
        async fn list(&self, _c: Option<&MemoryCategory>, _s: Option<&str>) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
        async fn forget(&self, _k: &str) -> anyhow::Result<bool> { Ok(true) }
        async fn count(&self) -> anyhow::Result<usize> { Ok(0) }
        async fn health_check(&self) -> bool { true }
        fn name(&self) -> &str { "noop" }
    }

    struct CaptureMem {
        stored: Mutex<Vec<(String, String, MemoryCategory, Option<String>)>>,
    }

    #[async_trait]
    impl Memory for CaptureMem {
        async fn store(&self, key: &str, content: &str, category: MemoryCategory, session_id: Option<&str>) -> anyhow::Result<()> {
            self.stored.lock().unwrap().push((key.to_string(), content.to_string(), category, session_id.map(String::from)));
            Ok(())
        }
        async fn recall(&self, _q: &str, _l: usize, _s: Option<&str>, _since: Option<&str>, _until: Option<&str>) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
        async fn get(&self, _k: &str) -> anyhow::Result<Option<MemoryEntry>> { Ok(None) }
        async fn list(&self, _c: Option<&MemoryCategory>, _s: Option<&str>) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
        async fn forget(&self, _k: &str) -> anyhow::Result<bool> { Ok(true) }
        async fn count(&self) -> anyhow::Result<usize> { Ok(self.stored.lock().unwrap().len()) }
        async fn health_check(&self) -> bool { true }
        fn name(&self) -> &str { "capture" }
    }

    struct FactExtractProvider;

    #[async_trait]
    impl Provider for FactExtractProvider {
        async fn chat_with_system(&self, _sp: Option<&str>, _m: &str, _model: &str, _t: f64) -> anyhow::Result<String> {
            Ok("[lang] User prefers Rust\n[db] PostgreSQL 16".to_string())
        }
        async fn chat(&self, _r: ChatRequest<'_>, _m: &str, _t: f64) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse { text: Some(String::new()), tool_calls: vec![], usage: None, reasoning_content: None })
        }
    }

    // ── parse_fact_line tests ───────────────────────────────────────

    #[test]
    fn parse_fact_line_extracts_key_and_content() {
        assert_eq!(
            parse_fact_line("[preferred_language] User prefers Rust over Go"),
            Some(("preferred_language", "User prefers Rust over Go"))
        );
    }

    #[test]
    fn parse_fact_line_handles_leading_dash() {
        assert_eq!(
            parse_fact_line("- [db_choice] Project uses PostgreSQL 16"),
            Some(("db_choice", "Project uses PostgreSQL 16"))
        );
    }

    #[test]
    fn parse_fact_line_rejects_empty_key_or_content() {
        assert_eq!(parse_fact_line("[] some content"), None);
        assert_eq!(parse_fact_line("[key]"), None);
        assert_eq!(parse_fact_line("[key]  "), None);
    }

    #[test]
    fn parse_fact_line_rejects_malformed_input() {
        assert_eq!(parse_fact_line("no brackets here"), None);
        assert_eq!(parse_fact_line(""), None);
        assert_eq!(parse_fact_line("[unclosed bracket"), None);
    }

    // ── normalize_fact_key tests ────────────────────────────────────

    #[test]
    fn normalize_fact_key_basic() {
        assert_eq!(normalize_fact_key("preferred_language"), "preferred_language");
        assert_eq!(normalize_fact_key("DB Choice"), "db_choice");
        assert_eq!(normalize_fact_key("my-cool-key"), "my_cool_key");
        assert_eq!(normalize_fact_key("  spaces  "), "spaces");
        assert_eq!(normalize_fact_key("UPPER_CASE"), "upper_case");
    }

    #[test]
    fn normalize_fact_key_collapses_underscores() {
        assert_eq!(normalize_fact_key("a___b"), "a_b");
        assert_eq!(normalize_fact_key("--key--"), "key");
    }

    #[test]
    fn normalize_fact_key_truncates_long_keys() {
        let long = "a".repeat(100);
        let result = normalize_fact_key(&long);
        assert_eq!(result.len(), FACT_KEY_MAX_LEN);
    }

    #[test]
    fn normalize_fact_key_empty_on_garbage() {
        assert_eq!(normalize_fact_key("!!!"), "");
        assert_eq!(normalize_fact_key(""), "");
    }

    // ── should_skip_redacted_fact tests ────────────────────────────

    #[test]
    fn skip_redacted_no_redaction_keeps_fact() {
        assert!(!should_skip_redacted_fact("User prefers Rust", "User prefers Rust"));
    }

    #[test]
    fn skip_redacted_empty_remainder_skips() {
        assert!(should_skip_redacted_fact("[REDACTED]", "sk-12345secret"));
    }

    #[test]
    fn skip_redacted_secret_shell_skips() {
        assert!(should_skip_redacted_fact("api key [REDACTED]", "api key sk-12345secret"));
        assert!(should_skip_redacted_fact("token: [REDACTED]", "token: abc123xyz"));
    }

    #[test]
    fn skip_redacted_meaningful_remainder_keeps() {
        assert!(!should_skip_redacted_fact(
            "User's deployment uses [REDACTED] for auth with PostgreSQL 16",
            "User's deployment uses sk-secret for auth with PostgreSQL 16"
        ));
    }

    // ── TurnBuffer tests ────────────────────────────────────────────

    #[test]
    fn turn_buffer_should_extract_requires_interval_and_chars() {
        let mut buf = TurnBuffer::new();
        assert!(!buf.should_extract());
        for i in 0..EXTRACT_TURN_INTERVAL {
            buf.push(&format!("q{i}"), "a");
        }
        assert!(!buf.should_extract()); // interval met but chars not
        let mut buf2 = TurnBuffer::new();
        let long_msg = "x".repeat(EXTRACT_MIN_CHARS);
        for _ in 0..EXTRACT_TURN_INTERVAL {
            buf2.push(&long_msg, "reply");
        }
        assert!(buf2.should_extract());
    }

    #[test]
    fn turn_buffer_drain_clears_and_marks_pending() {
        let mut buf = TurnBuffer::new();
        buf.push("hello", "world");
        assert!(!buf.is_empty());
        let turns = buf.drain_for_extraction();
        assert_eq!(turns.len(), 1);
        assert!(buf.is_empty());
        assert!(buf.needs_compaction_fallback());
    }

    #[test]
    fn turn_buffer_mark_success_clears_fallback() {
        let mut buf = TurnBuffer::new();
        buf.push("q", "a");
        let _ = buf.drain_for_extraction();
        assert!(buf.needs_compaction_fallback());
        buf.mark_extract_success();
        assert!(!buf.needs_compaction_fallback());
    }

    #[test]
    fn turn_buffer_needs_fallback_when_not_empty() {
        let mut buf = TurnBuffer::new();
        assert!(!buf.needs_compaction_fallback());
        buf.push("q", "a");
        assert!(buf.needs_compaction_fallback());
    }

    #[test]
    fn turn_buffer_counts_chars_not_bytes() {
        let mut buf = TurnBuffer::new();
        let cjk = "你".repeat(EXTRACT_MIN_CHARS);
        for _ in 0..EXTRACT_TURN_INTERVAL {
            buf.push(&cjk, "ok");
        }
        assert!(buf.should_extract());
    }

    // ── extract_facts_from_turns integration tests ─────────────────

    #[tokio::test]
    async fn extract_facts_stores_with_auto_prefix_and_core_category() {
        let mem = Arc::new(CaptureMem { stored: Mutex::new(Vec::new()) });
        let long_msg = "x".repeat(EXTRACT_MIN_CHARS);
        let turns = vec![(long_msg, "assistant reply".to_string())];
        let result = extract_facts_from_turns(&FactExtractProvider, "test-model", &turns, mem.as_ref(), Some("session-42")).await;
        assert_eq!(result.stored, 2);
        assert!(!result.no_facts);
        let stored = mem.stored.lock().unwrap();
        assert_eq!(stored[0].0, "auto_lang");
        assert_eq!(stored[0].1, "User prefers Rust");
        assert!(matches!(stored[0].2, MemoryCategory::Core));
        assert_eq!(stored[0].3, Some("session-42".to_string()));
        assert_eq!(stored[1].0, "auto_db");
    }

    #[tokio::test]
    async fn extract_facts_returns_no_facts_on_none_response() {
        struct NoneProvider;
        #[async_trait]
        impl Provider for NoneProvider {
            async fn chat_with_system(&self, _sp: Option<&str>, _m: &str, _model: &str, _t: f64) -> anyhow::Result<String> { Ok("NONE".to_string()) }
            async fn chat(&self, _r: ChatRequest<'_>, _m: &str, _t: f64) -> anyhow::Result<ChatResponse> {
                Ok(ChatResponse { text: Some(String::new()), tool_calls: vec![], usage: None, reasoning_content: None })
            }
        }
        let long_msg = "x".repeat(EXTRACT_MIN_CHARS);
        let turns = vec![(long_msg, "resp".to_string())];
        let result = extract_facts_from_turns(&NoneProvider, "model", &turns, &NoopMem, None).await;
        assert_eq!(result.stored, 0);
        assert!(result.no_facts);
    }

    #[tokio::test]
    async fn extract_facts_below_min_chars_returns_empty() {
        let turns = vec![("hi".to_string(), "hey".to_string())];
        let result = extract_facts_from_turns(&StaticSummaryProvider, "model", &turns, &NoopMem, None).await;
        assert_eq!(result.stored, 0);
        assert!(result.no_facts);
    }

    #[tokio::test]
    async fn extract_facts_unparseable_response_marks_no_facts_false() {
        struct GarbageProvider;
        #[async_trait]
        impl Provider for GarbageProvider {
            async fn chat_with_system(&self, _sp: Option<&str>, _m: &str, _model: &str, _t: f64) -> anyhow::Result<String> {
                Ok("This is just random text without any facts.".to_string())
            }
            async fn chat(&self, _r: ChatRequest<'_>, _m: &str, _t: f64) -> anyhow::Result<ChatResponse> {
                Ok(ChatResponse { text: Some(String::new()), tool_calls: vec![], usage: None, reasoning_content: None })
            }
        }
        let long_msg = "x".repeat(EXTRACT_MIN_CHARS);
        let turns = vec![(long_msg, "resp".to_string())];
        let result = extract_facts_from_turns(&GarbageProvider, "model", &turns, &NoopMem, None).await;
        assert_eq!(result.stored, 0);
        assert!(!result.no_facts, "unparseable LLM response must not mark extraction as successful");
    }

    #[tokio::test]
    async fn extract_facts_store_failure_marks_no_facts_false() {
        struct FailMem;
        #[async_trait]
        impl Memory for FailMem {
            async fn store(&self, _k: &str, _c: &str, _cat: MemoryCategory, _s: Option<&str>) -> anyhow::Result<()> { anyhow::bail!("disk full") }
            async fn recall(&self, _q: &str, _l: usize, _s: Option<&str>, _since: Option<&str>, _until: Option<&str>) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
            async fn get(&self, _k: &str) -> anyhow::Result<Option<MemoryEntry>> { Ok(None) }
            async fn list(&self, _c: Option<&MemoryCategory>, _s: Option<&str>) -> anyhow::Result<Vec<MemoryEntry>> { Ok(vec![]) }
            async fn forget(&self, _k: &str) -> anyhow::Result<bool> { Ok(true) }
            async fn count(&self) -> anyhow::Result<usize> { Ok(0) }
            async fn health_check(&self) -> bool { false }
            fn name(&self) -> &str { "fail" }
        }
        let long_msg = "x".repeat(EXTRACT_MIN_CHARS);
        let turns = vec![(long_msg, "resp".to_string())];
        let result = extract_facts_from_turns(&FactExtractProvider, "model", &turns, &FailMem, None).await;
        assert_eq!(result.stored, 0);
        assert!(!result.no_facts, "store failures must not mark extraction as successful");
    }

    // ── flush_durable_facts tests ───────────────────────────────────

    #[tokio::test]
    async fn flush_durable_facts_stores_core_facts() {
        let mem = Arc::new(CaptureMem { stored: Mutex::new(Vec::new()) });
        let transcript = "USER: What language?\nASSISTANT: User prefers Rust.";
        let ok = flush_durable_facts(&FactExtractProvider, "model", transcript, mem.as_ref(), None).await;
        assert!(ok);
        let stored = mem.stored.lock().unwrap();
        assert!(!stored.is_empty());
        assert!(stored[0].0.starts_with("auto_"));
        assert!(matches!(stored[0].2, MemoryCategory::Core));
    }

    #[tokio::test]
    async fn flush_durable_facts_returns_true_on_none() {
        struct NoneProvider;
        #[async_trait]
        impl Provider for NoneProvider {
            async fn chat_with_system(&self, _sp: Option<&str>, _m: &str, _model: &str, _t: f64) -> anyhow::Result<String> { Ok("NONE".to_string()) }
            async fn chat(&self, _r: ChatRequest<'_>, _m: &str, _t: f64) -> anyhow::Result<ChatResponse> {
                Ok(ChatResponse { text: Some(String::new()), tool_calls: vec![], usage: None, reasoning_content: None })
            }
        }
        let ok = flush_durable_facts(&NoneProvider, "model", "some transcript", &NoopMem, None).await;
        assert!(ok);
    }
}
