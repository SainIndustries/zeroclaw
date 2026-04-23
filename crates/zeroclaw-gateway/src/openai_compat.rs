//! OpenAI-compatible /v1/chat/completions adapter.
//!
//! Translates OpenAI chat-completion SSE in/out of upstream's
//! `Agent::turn_streamed` + `TurnEvent` event loop. Aura webapp speaks this
//! shape; internally we still run the full agent loop with tools + memory and
//! dispatch to whichever provider the agent is configured for (Bedrock in prod).

use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::Stream;
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
/// Only streaming (SSE) is supported — non-streaming requests return 501.
/// The full implementation is wired in a later task; this skeleton validates
/// the request and returns an immediately-closed stream.
pub async fn handle_chat_completions(
    State(_state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    if !req.stream {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            "non-streaming /v1/chat/completions is not supported".to_string(),
        ));
    }
    if req.messages.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "messages must not be empty".to_string(),
        ));
    }

    // Confirm there's at least one user message. We don't use it in the
    // skeleton, but validating now produces the right error shape for clients.
    let _user_message = req
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| m.content.clone())
        .ok_or((
            StatusCode::BAD_REQUEST,
            "no user message found in messages".to_string(),
        ))?;

    // Skeleton: immediately-closed channel so the stream terminates cleanly.
    // Task 12 replaces this with the real Agent::turn_streamed wiring.
    let (tx_placeholder, rx) = mpsc::channel::<SseFrame>(64);
    drop(tx_placeholder);

    let stream = ReceiverStream::new(rx).map(|frame: SseFrame| Ok(frame.to_event()));

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
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
}
