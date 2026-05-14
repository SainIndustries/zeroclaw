use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{debug, warn};
use uuid::Uuid;

use super::super::traits::HookHandler;
use zeroclaw_api::provider::ChatResponse;

/// Reports LLM token usage to Aura's local credential receiver for billing.
///
/// This hook is intentionally fail-closed. If the receiver denies usage or is
/// unreachable, the current turn stops before ZeroClaw continues to tools or
/// follow-up model calls.
pub struct UsageReporterHook {
    client: Client,
    port: u16,
    counter: AtomicU64,
}

impl UsageReporterHook {
    pub fn new() -> Self {
        Self::with_port(
            std::env::var("CRED_RECEIVER_PORT")
                .ok()
                .and_then(|port| port.parse().ok())
                .unwrap_or(18790),
        )
    }

    fn with_port(port: u16) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| Client::new()),
            port,
            counter: AtomicU64::new(0),
        }
    }

    async fn report_usage(&self, response: &ChatResponse) -> anyhow::Result<()> {
        let Some(usage) = &response.usage else {
            debug!("usage_reporter: provider response did not include usage");
            return Ok(());
        };

        let input_tokens = usage.input_tokens.unwrap_or(0);
        let output_tokens = usage.output_tokens.unwrap_or(0);
        let cached_input_tokens = usage.cached_input_tokens.unwrap_or(0);
        let total_tokens = input_tokens + output_tokens;

        if total_tokens == 0 {
            debug!("usage_reporter: provider response had zero billable tokens");
            return Ok(());
        }

        let seq = self.counter.fetch_add(1, Ordering::Relaxed);
        let report_id = format!("zc-{}-{seq}", Uuid::new_v4().as_simple());
        let url = format!("http://127.0.0.1:{}/report-usage", self.port);
        let payload = json!({
            "tokens": total_tokens,
            "channel": "vm",
            "reportId": report_id,
            "detail": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cached_input_tokens": cached_input_tokens,
            }
        });

        let response = self.client.post(&url).json(&payload).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(
                %status,
                body = %body,
                "usage_reporter: cred-receiver denied usage"
            );
            anyhow::bail!("cred-receiver denied usage report with status {status}");
        }

        debug!(tokens = total_tokens, "usage_reporter: usage accepted");
        Ok(())
    }
}

impl Default for UsageReporterHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HookHandler for UsageReporterHook {
    fn name(&self) -> &str {
        "usage_reporter"
    }

    async fn on_llm_output(&self, response: &ChatResponse) -> anyhow::Result<()> {
        self.report_usage(response).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, http::StatusCode, routing::post};
    use serde_json::Value;
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;
    use zeroclaw_api::provider::TokenUsage;

    fn response_with_usage(input_tokens: u64, output_tokens: u64) -> ChatResponse {
        ChatResponse {
            text: Some("ok".to_string()),
            tool_calls: Vec::new(),
            usage: Some(TokenUsage {
                input_tokens: Some(input_tokens),
                output_tokens: Some(output_tokens),
                cached_input_tokens: Some(7),
            }),
            reasoning_content: None,
        }
    }

    async fn start_receiver(status: StatusCode) -> (SocketAddr, Arc<Mutex<Vec<Value>>>) {
        let payloads = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&payloads);
        let app = Router::new().route(
            "/report-usage",
            post(move |Json(payload): Json<Value>| {
                let captured = Arc::clone(&captured);
                async move {
                    captured.lock().unwrap().push(payload);
                    status
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, payloads)
    }

    #[tokio::test]
    async fn reports_usage_to_cred_receiver() {
        let (addr, payloads) = start_receiver(StatusCode::OK).await;
        let hook = UsageReporterHook::with_port(addr.port());

        hook.on_llm_output(&response_with_usage(11, 13))
            .await
            .unwrap();

        let payloads = payloads.lock().unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["tokens"], 24);
        assert_eq!(payloads[0]["channel"], "vm");
        assert_eq!(payloads[0]["detail"]["input_tokens"], 11);
        assert_eq!(payloads[0]["detail"]["output_tokens"], 13);
        assert!(payloads[0]["reportId"].as_str().unwrap().starts_with("zc-"));
    }

    #[tokio::test]
    async fn fails_closed_when_cred_receiver_denies_usage() {
        let (addr, _) = start_receiver(StatusCode::PAYMENT_REQUIRED).await;
        let hook = UsageReporterHook::with_port(addr.port());

        let err = hook
            .on_llm_output(&response_with_usage(10, 5))
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("cred-receiver denied usage report")
        );
    }
}
