use super::traits::{LlmUsageAttribution, Observer, ObserverEvent, ObserverMetric};
use reqwest::Client;
use serde::Serialize;
use std::any::Any;

#[derive(Debug, Clone)]
struct AuraUsageConfig {
    api_url: String,
    agent_id: String,
    gateway_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuraUsagePayload {
    pub report_id: String,
    pub tokens: u64,
    pub channel: String,
    pub source: String,
    pub provider: String,
    pub model: String,
    pub metadata: AuraUsageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuraUsageMetadata {
    pub turn_id: String,
    pub iteration: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron_job_name: Option<String>,
}

pub struct AuraUsageObserver {
    config: Option<AuraUsageConfig>,
    client: Client,
}

impl AuraUsageObserver {
    pub fn from_env() -> Option<Self> {
        let Some(api_url) = std::env::var("AURA_API_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
        else {
            tracing::debug!("Aura usage reporting disabled: AURA_API_URL is not set");
            return None;
        };
        let Some(agent_id) = std::env::var("AGENT_ID")
            .ok()
            .map(|value| value.trim().to_string())
        else {
            tracing::debug!("Aura usage reporting disabled: AGENT_ID is not set");
            return None;
        };
        let Some(gateway_token) = std::env::var("GATEWAY_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
        else {
            tracing::debug!("Aura usage reporting disabled: GATEWAY_TOKEN is not set");
            return None;
        };
        if api_url.is_empty() || agent_id.is_empty() || gateway_token.is_empty() {
            tracing::debug!("Aura usage reporting disabled: one or more required values are empty");
            return None;
        }
        Some(Self {
            config: Some(AuraUsageConfig {
                api_url,
                agent_id,
                gateway_token,
            }),
            client: Client::new(),
        })
    }

    #[cfg(test)]
    fn disabled_for_test() -> Self {
        Self {
            config: None,
            client: Client::new(),
        }
    }

    #[cfg(test)]
    fn enabled_for_test(api_url: &str, agent_id: &str, gateway_token: &str) -> Self {
        Self {
            config: Some(AuraUsageConfig {
                api_url: api_url.trim_end_matches('/').to_string(),
                agent_id: agent_id.to_string(),
                gateway_token: gateway_token.to_string(),
            }),
            client: Client::new(),
        }
    }

    fn payload_for_event(&self, event: &ObserverEvent) -> Option<AuraUsagePayload> {
        let ObserverEvent::LlmResponse {
            provider,
            model,
            success: true,
            input_tokens,
            output_tokens,
            usage_attribution: Some(attribution),
            ..
        } = event
        else {
            return None;
        };

        let tokens = input_tokens
            .unwrap_or(0)
            .saturating_add(output_tokens.unwrap_or(0));
        if tokens == 0 {
            return None;
        }

        Some(build_payload(provider, model, tokens, attribution))
    }

    fn report_url(&self) -> Option<String> {
        let config = self.config.as_ref()?;
        Some(format!(
            "{}/api/internal/agents/{}/usage",
            config.api_url, config.agent_id
        ))
    }
}

fn build_payload(
    provider: &str,
    model: &str,
    tokens: u64,
    attribution: &LlmUsageAttribution,
) -> AuraUsagePayload {
    let cron_part = attribution.cron_job_id.as_deref().unwrap_or("none");
    let report_id = format!(
        "zeroclaw:{}:{}:{}:{}:{}:{}",
        attribution.turn_id, attribution.iteration, provider, model, attribution.channel, cron_part
    );

    AuraUsagePayload {
        report_id,
        tokens,
        channel: attribution.channel.clone(),
        source: attribution.source.clone(),
        provider: provider.to_string(),
        model: model.to_string(),
        metadata: AuraUsageMetadata {
            turn_id: attribution.turn_id.clone(),
            iteration: attribution.iteration,
            cron_job_id: attribution.cron_job_id.clone(),
            cron_job_name: attribution.cron_job_name.clone(),
        },
    }
}

impl Observer for AuraUsageObserver {
    fn record_event(&self, event: &ObserverEvent) {
        let Some(config) = self.config.clone() else {
            return;
        };
        let Some(payload) = self.payload_for_event(event) else {
            return;
        };
        let Some(url) = self.report_url() else {
            return;
        };
        let client = self.client.clone();

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            tracing::warn!(
                report_id = %payload.report_id,
                "Aura usage report skipped because no Tokio runtime is active"
            );
            return;
        };

        handle.spawn(async move {
            match client
                .post(url)
                .bearer_auth(config.gateway_token)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {}
                Ok(response) => {
                    tracing::warn!(
                        status = %response.status(),
                        report_id = %payload.report_id,
                        "Aura usage report was rejected"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        report_id = %payload.report_id,
                        error = %error,
                        "Aura usage report failed"
                    );
                }
            }
        });
    }

    fn record_metric(&self, _metric: &ObserverMetric) {}

    fn name(&self) -> &str {
        "aura_usage"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn success_event(attribution: LlmUsageAttribution) -> ObserverEvent {
        ObserverEvent::LlmResponse {
            provider: "bedrock".to_string(),
            model: "claude-sonnet".to_string(),
            duration: Duration::from_millis(42),
            success: true,
            error_message: None,
            input_tokens: Some(100),
            output_tokens: Some(25),
            usage_attribution: Some(attribution),
        }
    }

    fn attribution() -> LlmUsageAttribution {
        LlmUsageAttribution {
            turn_id: "turn-1".to_string(),
            iteration: 2,
            channel: "proactive".to_string(),
            source: "zeroclaw_cron".to_string(),
            cron_job_id: Some("job-1".to_string()),
            cron_job_name: Some("Morning briefing".to_string()),
        }
    }

    #[test]
    fn reports_usage_on_successful_llm_response() {
        let observer = AuraUsageObserver::enabled_for_test(
            "https://www.moreaura.ai/",
            "agent-1",
            "gateway-token",
        );
        let payload = observer
            .payload_for_event(&success_event(attribution()))
            .expect("successful token response should build a payload");

        assert_eq!(payload.tokens, 125);
        assert_eq!(payload.channel, "proactive");
        assert_eq!(payload.source, "zeroclaw_cron");
        assert_eq!(payload.provider, "bedrock");
        assert_eq!(payload.model, "claude-sonnet");
        assert_eq!(payload.metadata.cron_job_id.as_deref(), Some("job-1"));
        assert_eq!(
            payload.report_id,
            "zeroclaw:turn-1:2:bedrock:claude-sonnet:proactive:job-1"
        );
        assert_eq!(
            observer.report_url().as_deref(),
            Some("https://www.moreaura.ai/api/internal/agents/agent-1/usage")
        );
    }

    #[test]
    fn skips_when_env_config_is_missing() {
        let observer = AuraUsageObserver::disabled_for_test();
        assert!(observer.report_url().is_none());
        assert!(observer
            .payload_for_event(&success_event(attribution()))
            .is_some());
    }

    #[test]
    fn skips_failed_and_zero_token_responses() {
        let observer = AuraUsageObserver::enabled_for_test("https://aura.test", "agent-1", "token");
        let failed = ObserverEvent::LlmResponse {
            provider: "bedrock".to_string(),
            model: "claude-sonnet".to_string(),
            duration: Duration::from_millis(42),
            success: false,
            error_message: Some("failed".to_string()),
            input_tokens: Some(100),
            output_tokens: Some(25),
            usage_attribution: Some(attribution()),
        };
        assert!(observer.payload_for_event(&failed).is_none());

        let zero = ObserverEvent::LlmResponse {
            provider: "bedrock".to_string(),
            model: "claude-sonnet".to_string(),
            duration: Duration::from_millis(42),
            success: true,
            error_message: None,
            input_tokens: None,
            output_tokens: None,
            usage_attribution: Some(attribution()),
        };
        assert!(observer.payload_for_event(&zero).is_none());
    }

    #[test]
    fn deterministic_report_id_is_stable_for_same_event() {
        let first = build_payload("bedrock", "claude-sonnet", 125, &attribution());
        let second = build_payload("bedrock", "claude-sonnet", 125, &attribution());
        assert_eq!(first.report_id, second.report_id);
    }
}
