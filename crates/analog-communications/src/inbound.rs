use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::identity::IdentityLevel;
use crate::sanitization;

/// Telnyx inbound SMS webhook handler.
///
/// POST /sms-inbound
/// Receives Telnyx webhook events, verifies signature, sanitizes,
/// resolves sender identity, and dispatches to pipeline.
pub async fn sms_inbound(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(payload): Json<TelnyxWebhook>,
) -> impl IntoResponse {
    // 1. Verify Telnyx webhook signature (Ed25519)
    if state.config.telnyx_public_key.is_some() {
        let signature = headers
            .get("telnyx-signature-ed25519")
            .and_then(|v| v.to_str().ok());
        let timestamp = headers
            .get("telnyx-timestamp")
            .and_then(|v| v.to_str().ok());

        if signature.is_none() || timestamp.is_none() {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "missing_signature" })),
            )
                .into_response();
        }

        // TODO: Actual Ed25519 verification against public key
        // For now, presence check is the gate
    }

    // 2. Extract message data
    let event = match &payload.data {
        Some(data) => data,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "missing_data" })),
            )
                .into_response();
        }
    };

    let from = event.payload.from.number.as_deref().unwrap_or("");
    let body = event.payload.text.as_deref().unwrap_or("");

    // 3. Sanitize input
    let sanitized = match sanitization::sanitize_sms(from, body) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(from = %from, error = %e, "SMS sanitization rejected");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "sanitization_failed", "detail": e.to_string() })),
            )
                .into_response();
        }
    };

    // 4. Resolve sender identity
    let identity_level = crate::identity::resolve_sender(
        from,
        &state.config.allowed_senders,
        state.config.owner_number.as_deref(),
    );

    if identity_level == IdentityLevel::Unknown {
        tracing::warn!(from = %from, "Unknown sender — quarantined");
        return (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "quarantined",
                "reason": "unknown_sender",
            })),
        )
            .into_response();
    }

    // 5. Dispatch to pipeline
    let dispatch_result = crate::dispatch::dispatch_to_pipeline(
        &state.http,
        &state.config.cortex_url,
        &sanitized,
        identity_level,
    )
    .await;

    match dispatch_result {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "accepted",
                "from": from,
                "identity_level": format!("{identity_level:?}"),
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Pipeline dispatch failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "dispatch_failed" })),
            )
                .into_response()
        }
    }
}

// Telnyx webhook types (simplified)

#[derive(Debug, Deserialize)]
pub struct TelnyxWebhook {
    pub data: Option<TelnyxEvent>,
}

#[derive(Debug, Deserialize)]
pub struct TelnyxEvent {
    pub event_type: Option<String>,
    pub payload: TelnyxPayload,
}

#[derive(Debug, Deserialize)]
pub struct TelnyxPayload {
    pub from: TelnyxAddress,
    pub to: Vec<TelnyxAddress>,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelnyxAddress {
    pub number: Option<String>,
}

/// Sanitized inbound message — safe for downstream processing.
#[derive(Debug, Clone, Serialize)]
pub struct SanitizedMessage {
    pub from: String,
    pub body: String,
    pub received_at: String,
    pub message_id: String,
    pub labels: Vec<String>,
}
