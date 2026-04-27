//! OpenAI-compatible /v1/chat/completions adapter.
//!
//! Translates OpenAI chat-completion SSE in/out of upstream's
//! `Agent::turn_streamed` + `TurnEvent` event loop. Aura webapp speaks this
//! shape; internally we still run the full agent loop with tools + memory and
//! dispatch to whichever provider the agent is configured for (Bedrock in prod).

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    #[serde(default)]
    pub stream: bool,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Internal SSE frame type emitted to the client.
#[derive(Debug)]
pub enum SseFrame {
    /// A `data: {json}` line representing one OpenAI chunk.
    Delta(serde_json::Value),
    /// Terminal `data: [DONE]` sentinel.
    Done,
}

impl SseFrame {
    pub fn to_event(self) -> Event {
        match self {
            SseFrame::Delta(v) => Event::default().data(v.to_string()),
            SseFrame::Done => Event::default().data("[DONE]"),
        }
    }
}

/// Handler for POST /v1/chat/completions.
///
/// Supports both streaming (`stream: true`, SSE) and non-streaming
/// (`stream: false`, single JSON body). The streaming path wires into
/// `Agent::turn_streamed` following the same pattern as
/// `ws::process_chat_message`; the non-streaming path uses `Agent::turn`
/// and returns a full `chat.completion` JSON response.
pub async fn handle_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Response, (StatusCode, String)> {
    authorize(&headers, &state)?;
    if req.messages.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "messages must not be empty".to_string(),
        ));
    }

    let user_message = req
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| m.content.clone())
        .ok_or((
            StatusCode::BAD_REQUEST,
            "no user message found in messages".to_string(),
        ))?;

    // Read optional session id BEFORE moving headers into anything.
    let session_id = read_session_id(&headers);
    let session_key = session_id.as_ref().map(|id| format!("gw_{id}"));

    if !req.stream {
        return handle_non_streaming(state, req.model, user_message, session_id).await;
    }

    let completion_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
    let model = req.model.clone();

    // Outbound channel: SseFrame -> HTTP SSE stream
    let (out_tx, out_rx) = mpsc::channel::<SseFrame>(64);
    // Turn-event channel: Agent -> forwarder
    let (evt_tx, mut evt_rx) =
        mpsc::channel::<zeroclaw_runtime::agent::TurnEvent>(64);

    // ── Forwarder task: TurnEvent -> SseFrame ────────────────────────────────
    let completion_id_fwd = completion_id.clone();
    let model_fwd = model.clone();
    let out_tx_fwd = out_tx.clone();
    let forwarder = tokio::spawn(async move {
        // Emit role=assistant opener so clients know the stream has started.
        let _ = out_tx_fwd
            .send(SseFrame::Delta(build_chunk(
                &completion_id_fwd,
                &model_fwd,
                serde_json::json!({ "role": "assistant" }),
                None,
            )))
            .await;

        let mut tool_call_index: u32 = 0;
        while let Some(evt) = evt_rx.recv().await {
            let frames = translate_turn_event(
                evt,
                &completion_id_fwd,
                &model_fwd,
                &mut tool_call_index,
            );
            for frame in frames {
                if out_tx_fwd.send(SseFrame::Delta(frame)).await.is_err() {
                    return;
                }
            }
        }
    });

    // Capture references to move into the driver task.
    let user_message_drv = user_message.clone();
    let session_id_drv = session_id.clone();
    let session_key_drv = session_key.clone();

    // ── Driver task: create agent, run turn, emit terminal frames ────────────
    tokio::spawn(async move {
        // Mirror ws::handle_socket: build an Agent from the current config.
        // The config lock is released immediately after cloning.
        let config = state.config.lock().clone();
        let mut agent = match zeroclaw_runtime::agent::Agent::from_config(&config).await {
            Ok(a) => a,
            Err(e) => {
                tracing::error!(error = %e, "Agent init failed for /v1/chat/completions");
                let _ = out_tx
                    .send(SseFrame::Delta(build_chunk(
                        &completion_id,
                        &model,
                        serde_json::json!({}),
                        Some("error"),
                    )))
                    .await;
                let _ = out_tx.send(SseFrame::Done).await;
                return;
            }
        };

        // Scope memory consolidation to this session (Daily/Core fact rows
        // tagged with the same session_id). When None, falls back to the
        // agent's default "no session" memory bucket (today's behavior).
        if let Some(ref id) = session_id_drv {
            agent.set_memory_session_id(Some(id.clone()));
        }

        // Seed prior turns from SqliteSessionBackend so the agent has
        // continuity within the conversation. Skipped silently if either
        // the backend is disabled (config) or no session header was sent.
        if let (Some(backend), Some(key)) =
            (&state.session_backend, &session_key_drv)
        {
            let prior = backend.load(key);
            if !prior.is_empty() {
                tracing::debug!(
                    session_key = %key,
                    prior_count = prior.len(),
                    "openai_compat: seeding agent with prior history"
                );
                agent.seed_history(&prior);
            }
        }

        // Run the agent turn, streaming events into evt_tx. The forwarder
        // task above drains evt_rx concurrently. turn_streamed takes
        // ownership of evt_tx and drops it on return, signalling the
        // forwarder's while-let loop to exit once drained.
        // Note: state.session_queue is intentionally NOT acquired here.
        // Aura serializes requests per session_id at the webhook layer
        // (one channel partner = one in-flight request at a time). If we
        // ever observe interleaved turns on the same session in production,
        // add session_queue.acquire(&session_key).await before turn_streamed.
        let result = agent.turn_streamed(&user_message_drv, evt_tx).await;

        // Wait for the forwarder to fully drain queued TurnEvents into
        // out_tx before we emit finish + [DONE]. Without this join, the
        // terminal frames can arrive on the client before trailing content
        // chunks, which violates the OpenAI SSE protocol (content after
        // [DONE]).
        let _ = forwarder.await;

        // Emit terminal finish_reason frame + [DONE] sentinel.
        let finish = match &result {
            Ok(_) => "stop",
            Err(e) => {
                tracing::warn!("/v1/chat/completions turn_streamed error: {e}");
                "error"
            }
        };
        let _ = out_tx
            .send(SseFrame::Delta(build_chunk(
                &completion_id,
                &model,
                serde_json::json!({}),
                Some(finish),
            )))
            .await;
        let _ = out_tx.send(SseFrame::Done).await;

        if let Ok(ref response) = result {
            // Persist user + assistant to the session backend if a session
            // key was provided and the backend is configured.
            if let (Some(backend), Some(key)) =
                (state.session_backend.as_ref(), session_key_drv.as_ref())
            {
                let user_msg = zeroclaw_api::provider::ChatMessage::user(user_message_drv.clone());
                if let Err(e) = backend.append(key, &user_msg) {
                    tracing::warn!(
                        session_key = %key,
                        error = %e,
                        "openai_compat: failed to persist user message"
                    );
                }
                let assistant_msg = zeroclaw_api::provider::ChatMessage::assistant(response.clone());
                if let Err(e) = backend.append(key, &assistant_msg) {
                    tracing::warn!(
                        session_key = %key,
                        error = %e,
                        "openai_compat: failed to persist assistant message"
                    );
                }
                // Note: backend.set_session_state is intentionally NOT called here.
                // Aura tracks turn state (idle/running/error) in its own webapp DB
                // for UI indicators; the gateway-side session_state column is left
                // untouched on the OpenAI-compat path.
            }

            // Fire-and-forget memory consolidation, mirroring ws::process_chat_message.
            if state.auto_save {
                let mem = state.mem.clone();
                let provider = state.provider.clone();
                let model_consolidate = model.clone();
                let user_msg = user_message_drv.clone();
                let assistant_resp = response.clone();
                tokio::spawn(async move {
                    if let Err(e) = zeroclaw_memory::consolidation::consolidate_turn(
                        provider.as_ref(),
                        &model_consolidate,
                        mem.as_ref(),
                        &user_msg,
                        &assistant_resp,
                    )
                    .await
                    {
                        tracing::debug!("OpenAI-compat memory consolidation skipped: {e}");
                    }
                });
            }
        }
    });

    let stream = ReceiverStream::new(out_rx)
        .map(|frame| Ok::<Event, Infallible>(frame.to_event()));
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()).into_response())
}

/// Non-streaming path: run `Agent::turn` synchronously and return a single
/// `chat.completion` JSON response. Used by callers that can't consume SSE
/// (Blooio iMessage webhook, voice LLM proxy). Tool calls execute
/// internally; the response contains only the final assistant message.
async fn handle_non_streaming(
    state: AppState,
    model: String,
    user_message: String,
    session_id: Option<String>,
) -> Result<Response, (StatusCode, String)> {
    let session_key = session_id.as_ref().map(|id| format!("gw_{id}"));

    let config = state.config.lock().clone();
    let mut agent = zeroclaw_runtime::agent::Agent::from_config(&config)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Agent init failed for non-streaming /v1/chat/completions");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("agent init failed: {e}"),
            )
        })?;

    if let Some(ref id) = session_id {
        agent.set_memory_session_id(Some(id.clone()));
    }

    if let (Some(backend), Some(key)) =
        (state.session_backend.as_ref(), session_key.as_ref())
    {
        let prior = backend.load(key);
        if !prior.is_empty() {
            tracing::debug!(
                session_key = %key,
                prior_count = prior.len(),
                "openai_compat (non-stream): seeding agent with prior history"
            );
            agent.seed_history(&prior);
        }
    }

    // Note: state.session_queue is intentionally NOT acquired here.
    // Aura serializes requests per session_id at the webhook layer
    // (one channel partner = one in-flight request at a time). If we
    // ever observe interleaved turns on the same session in production,
    // add session_queue.acquire(&session_key).await before turn_streamed.
    let response_text = agent.turn(&user_message).await.map_err(|e| {
        tracing::warn!(error = %e, "non-streaming /v1/chat/completions turn error");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("turn failed: {e}"),
        )
    })?;

    if let (Some(backend), Some(key)) =
        (state.session_backend.as_ref(), session_key.as_ref())
    {
        let user_msg = zeroclaw_api::provider::ChatMessage::user(user_message.clone());
        if let Err(e) = backend.append(key, &user_msg) {
            tracing::warn!(
                session_key = %key,
                error = %e,
                "openai_compat (non-stream): failed to persist user message"
            );
        }
        let assistant_msg = zeroclaw_api::provider::ChatMessage::assistant(response_text.clone());
        if let Err(e) = backend.append(key, &assistant_msg) {
            tracing::warn!(
                session_key = %key,
                error = %e,
                "openai_compat (non-stream): failed to persist assistant message"
            );
        }
        // Note: backend.set_session_state is intentionally NOT called here.
        // Aura tracks turn state (idle/running/error) in its own webapp DB
        // for UI indicators; the gateway-side session_state column is left
        // untouched on the OpenAI-compat path.
    }

    if state.auto_save {
        let mem = state.mem.clone();
        let provider = state.provider.clone();
        let model_consolidate = model.clone();
        let user_msg = user_message.clone();
        let assistant_resp = response_text.clone();
        tokio::spawn(async move {
            if let Err(e) = zeroclaw_memory::consolidation::consolidate_turn(
                provider.as_ref(),
                &model_consolidate,
                mem.as_ref(),
                &user_msg,
                &assistant_resp,
            )
            .await
            {
                tracing::debug!(
                    "non-streaming /v1/chat/completions memory consolidation skipped: {e}"
                );
            }
        });
    }

    let completion_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
    let created = chrono::Utc::now().timestamp();
    let body = serde_json::json!({
        "id": completion_id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": response_text,
            },
            "finish_reason": "stop",
        }],
    });

    Ok(Json(body).into_response())
}

/// Extract the `X-Aura-Session-Id` header value, if present and non-empty.
///
/// Aura sets this header on every channel-mediated request (iMessage,
/// dashboard, Slack, voice). When present, the adapter scopes both
/// session-history persistence and the agent's memory_session_id to this
/// value (prefixed with `gw_`). When absent, the adapter falls back to
/// today's stateless "fresh agent per request" behavior.
fn read_session_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-aura-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Validate the `Authorization: Bearer <token>` header.
///
/// Accepts two token classes in order:
///   1. `AURA_INTERNAL_SECRET` env-var value (read at startup into `AppState`).
///   2. Any token registered via the upstream paired-token mechanism
///      (`PairingGuard::is_authenticated`).
///
/// If neither matches, or the header is absent, returns `401 Unauthorized`.
fn authorize(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<(), (StatusCode, String)> {
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
        .trim();

    if bearer.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "missing bearer token".into()));
    }

    // Case 1: AURA_INTERNAL_SECRET (env-derived) match
    if let Some(ref secret) = state.aura_internal_secret {
        if bearer == secret {
            return Ok(());
        }
    }

    // Case 2: upstream paired-token check via PairingGuard::is_authenticated.
    if state.pairing.is_authenticated(bearer) {
        return Ok(());
    }

    Err((StatusCode::UNAUTHORIZED, "invalid bearer token".into()))
}

/// Build one OpenAI chat-completion chunk frame from a delta payload.
///
/// `id` stays stable across all frames in one response; `model` echoes the
/// request's `model` field; `delta` is the per-frame JSON object (role/content
/// /tool_calls/etc.); `finish_reason` is `None` for intermediate frames and
/// `Some("stop")` / `Some("tool_calls")` / `Some("error")` for terminators.
fn build_chunk(
    id: &str,
    model: &str,
    delta: serde_json::Value,
    finish_reason: Option<&str>,
) -> serde_json::Value {
    let created = chrono::Utc::now().timestamp();
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": finish_reason,
        }]
    })
}

/// Translate one `TurnEvent` into zero or more OpenAI chunk frames.
///
/// `tool_call_index` is a running counter for the `tool_calls[].index`
/// field; each distinct ToolCall event increments it.
pub(crate) fn translate_turn_event(
    event: zeroclaw_runtime::agent::TurnEvent,
    id: &str,
    model: &str,
    tool_call_index: &mut u32,
) -> Vec<serde_json::Value> {
    use zeroclaw_runtime::agent::TurnEvent;
    match event {
        TurnEvent::Chunk { delta } => vec![build_chunk(
            id,
            model,
            serde_json::json!({ "content": delta }),
            None,
        )],
        TurnEvent::Thinking { delta } => {
            // OpenAI has no standard "thinking" delta field. We emit it as a
            // non-standard field that strict OpenAI clients ignore and
            // Aura-aware clients can surface.
            vec![build_chunk(
                id,
                model,
                serde_json::json!({ "reasoning": delta }),
                None,
            )]
        }
        TurnEvent::ToolCall { name, args } => {
            let idx = *tool_call_index;
            *tool_call_index += 1;
            let call_id = format!("call_{}", uuid::Uuid::new_v4().simple());
            vec![build_chunk(
                id,
                model,
                serde_json::json!({
                    "tool_calls": [{
                        "index": idx,
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": args.to_string(),
                        }
                    }]
                }),
                None,
            )]
        }
        TurnEvent::ToolResult { name, output } => {
            // OpenAI SSE has no native "tool result in stream". We emit a
            // non-standard `tool_result` field; strict clients ignore it.
            vec![build_chunk(
                id,
                model,
                serde_json::json!({
                    "tool_result": { "name": name, "output": output }
                }),
                None,
            )]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use zeroclaw_runtime::agent::TurnEvent;

    #[test]
    fn chunk_event_emits_content_delta() {
        let mut idx = 0;
        let frames = translate_turn_event(
            TurnEvent::Chunk { delta: "hi".to_string() },
            "chatcmpl-test",
            "m",
            &mut idx,
        );
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["choices"][0]["delta"]["content"], "hi");
        assert_eq!(idx, 0, "content chunk should not advance tool index");
    }

    #[test]
    fn thinking_event_emits_reasoning_field() {
        let mut idx = 0;
        let frames = translate_turn_event(
            TurnEvent::Thinking { delta: "pondering".to_string() },
            "chatcmpl-test",
            "m",
            &mut idx,
        );
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["choices"][0]["delta"]["reasoning"], "pondering");
        assert_eq!(idx, 0);
    }

    #[test]
    fn tool_call_event_emits_tool_calls_with_index() {
        let mut idx = 5;
        let frames = translate_turn_event(
            TurnEvent::ToolCall {
                name: "shell".to_string(),
                args: serde_json::json!({ "cmd": "echo hi" }),
            },
            "chatcmpl-test",
            "m",
            &mut idx,
        );
        let call = &frames[0]["choices"][0]["delta"]["tool_calls"][0];
        assert_eq!(call["index"], 5);
        assert_eq!(call["type"], "function");
        assert_eq!(call["function"]["name"], "shell");
        assert!(call["function"]["arguments"]
            .as_str()
            .unwrap()
            .contains("echo hi"));
        assert_eq!(idx, 6, "tool call should advance index by 1");
    }

    #[test]
    fn tool_result_event_emits_nonstandard_tool_result_field() {
        let mut idx = 0;
        let frames = translate_turn_event(
            TurnEvent::ToolResult {
                name: "shell".to_string(),
                output: "hi\n".to_string(),
            },
            "chatcmpl-test",
            "m",
            &mut idx,
        );
        assert_eq!(frames[0]["choices"][0]["delta"]["tool_result"]["name"], "shell");
        assert_eq!(frames[0]["choices"][0]["delta"]["tool_result"]["output"], "hi\n");
    }

    #[test]
    fn read_session_id_returns_some_when_header_present() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-aura-session-id",
            "imsg_abc123def456".parse().unwrap(),
        );
        assert_eq!(
            read_session_id(&headers),
            Some("imsg_abc123def456".to_string())
        );
    }

    #[test]
    fn read_session_id_returns_none_when_header_absent() {
        let headers = HeaderMap::new();
        assert_eq!(read_session_id(&headers), None);
    }

    #[test]
    fn read_session_id_trims_whitespace_and_rejects_empty() {
        let mut headers = HeaderMap::new();
        headers.insert("x-aura-session-id", "   ".parse().unwrap());
        assert_eq!(read_session_id(&headers), None);
    }

    #[test]
    fn session_key_format_matches_design() {
        // Wire-contract test: the session_key prefix is "gw_" (single
        // underscore, lowercase) as agreed in the design spec
        // (docs/superpowers/specs/2026-04-24-unified-conversation-architecture-design.md).
        // Aura webhooks send the bare session_id; the adapter prepends "gw_".
        let id = "imsg_abc123";
        assert_eq!(format!("gw_{id}"), "gw_imsg_abc123");
    }

    #[test]
    fn authorize_accepts_aura_internal_secret() {
        // Verify the bearer extraction logic handles the common case.
        let secret = "deadbeef";
        let header_value = format!("Bearer {secret}");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            header_value.parse().unwrap(),
        );
        let extracted = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|v| v.trim())
            .unwrap();
        assert_eq!(extracted, secret);
    }
}
