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
