use async_trait::async_trait;
use reqwest::Client;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::providers::traits::ChatResponse;

use super::super::traits::HookHandler;

/// Reports LLM token usage to the local credential receiver for billing.
///
/// Fires on every `on_llm_output` event. Posts usage data asynchronously
/// to `http://127.0.0.1:{port}/report-usage` (fire-and-forget).
pub struct UsageReporterHook {
    client: Client,
    port: u16,
    counter: AtomicU64,
}

impl UsageReporterHook {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            port: std::env::var("CRED_RECEIVER_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(18790),
            counter: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl HookHandler for UsageReporterHook {
    fn name(&self) -> &str {
        "usage_reporter"
    }

    async fn on_llm_output(&self, response: &ChatResponse) {
        let usage = match &response.usage {
            Some(u) => u,
            None => return, // No usage data — skip
        };

        let input = usage.input_tokens.unwrap_or(0);
        let output = usage.output_tokens.unwrap_or(0);
        let total_tokens = input + output;
        if total_tokens == 0 {
            return;
        }

        // Generate a unique report ID for idempotency
        let seq = self.counter.fetch_add(1, Ordering::Relaxed);
        let report_id = format!("zc-{}-{}", Uuid::new_v4().as_simple(), seq);

        let url = format!("http://127.0.0.1:{}/report-usage", self.port);

        let payload = serde_json::json!({
            "tokens": total_tokens,
            "channel": "vm",
            "reportId": report_id,
            "detail": {
                "input_tokens": input,
                "output_tokens": output,
            }
        });

        debug!(
            tokens = total_tokens,
            input = input,
            output = output,
            "Reporting LLM usage to cred-receiver"
        );

        // Fire-and-forget — don't block the agent loop
        let client = self.client.clone();
        tokio::spawn(async move {
            match client.post(&url).json(&payload).send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!("Usage reported: {} tokens", total_tokens);
                }
                Ok(resp) => {
                    warn!(
                        status = %resp.status(),
                        "Usage report returned non-success status"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "Failed to report usage to cred-receiver");
                }
            }
        });
    }
}
