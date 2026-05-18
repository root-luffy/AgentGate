use reqwest::Client;
use std::time::Duration;

use crate::error::AppError;

#[derive(Clone)]
pub struct Forwarder {
    client: Client,
}

impl Forwarder {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_millis(timeout_ms))
                .build()
                .expect("failed to build reqwest client"),
        }
    }

    pub async fn forward_json(
        &self,
        upstream_url: &str,
        body: bytes::Bytes,
        agent_id: uuid::Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let response = self
            .client
            .post(upstream_url)
            .header("Content-Type", "application/json")
            .header("X-AgentGate-Agent-Id", agent_id.to_string())
            .body(body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    AppError::Internal(anyhow::anyhow!("upstream timeout"))
                } else {
                    AppError::BadGateway
                }
            })?;

        if !response.status().is_success() {
            return Err(AppError::BadGateway);
        }

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|_| AppError::BadGateway)
    }
}
