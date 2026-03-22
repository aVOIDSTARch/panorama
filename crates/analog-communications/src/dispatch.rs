use crate::identity::IdentityLevel;
use crate::inbound::SanitizedMessage;

/// Dispatch a sanitized message to the appropriate pipeline via Cortex.
///
/// Scope limits are explicit:
/// - Known senders trigger IDEA_CAPTURE to Cerebro
/// - Owner senders can additionally trigger Wheelhouse tasks
/// - Other levels are quarantined (handled before this function)
pub async fn dispatch_to_pipeline(
    http: &reqwest::Client,
    cortex_url: &str,
    message: &SanitizedMessage,
    identity_level: IdentityLevel,
) -> Result<(), DispatchError> {
    match identity_level {
        IdentityLevel::Owner | IdentityLevel::Known => {
            // IDEA_CAPTURE: send to Cerebro via Cortex for knowledge graph ingestion
            let url = format!("{cortex_url}/cerebro/quarantine/ingest");
            let payload = serde_json::json!({
                "source": "sms",
                "sender": message.from,
                "content": message.body,
                "labels": message.labels,
                "message_id": message.message_id,
                "received_at": message.received_at,
            });

            let resp = http
                .post(&url)
                .json(&payload)
                .send()
                .await
                .map_err(|e| DispatchError::NetworkError(e.to_string()))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(DispatchError::DownstreamError {
                    service: "cerebro".into(),
                    status: status.as_u16(),
                    body,
                });
            }

            tracing::info!(
                message_id = %message.message_id,
                from = %message.from,
                "IDEA_CAPTURE dispatched to Cerebro"
            );
            Ok(())
        }
        _ => {
            // Should not reach here (quarantined upstream), but be safe
            Err(DispatchError::Unauthorized)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Downstream service error ({service} HTTP {status}): {body}")]
    DownstreamError {
        service: String,
        status: u16,
        body: String,
    },
    #[error("Unauthorized sender")]
    Unauthorized,
}

impl From<DispatchError> for panorama_errors::PanoramaError {
    fn from(err: DispatchError) -> Self {
        let (code, detail) = match &err {
            DispatchError::NetworkError(d) => ("AC-001", Some(d.clone())),
            DispatchError::DownstreamError {
                service,
                status,
                body,
            } => ("AC-002", Some(format!("{service} HTTP {status}: {body}"))),
            DispatchError::Unauthorized => ("AC-003", None),
        };
        panorama_errors::PanoramaError::from_code(code, "analog-communications", detail)
    }
}
