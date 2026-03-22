use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
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
    body: Bytes,
) -> impl IntoResponse {
    // 1. Verify Telnyx webhook signature (Ed25519)
    if let Some(ref public_key_b64) = state.config.telnyx_public_key {
        let signature = headers
            .get("telnyx-signature-ed25519")
            .and_then(|v| v.to_str().ok());
        let timestamp = headers
            .get("telnyx-timestamp")
            .and_then(|v| v.to_str().ok());

        match (signature, timestamp) {
            (Some(sig_b64), Some(ts)) => {
                if let Err(e) = verify_telnyx_signature(public_key_b64, sig_b64, ts, &body) {
                    tracing::warn!(error = %e, "Telnyx signature verification failed");
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({ "error": "invalid_signature", "detail": e })),
                    )
                        .into_response();
                }
            }
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "missing_signature" })),
                )
                    .into_response();
            }
        }
    }

    // Parse body as JSON after signature verification
    let payload: TelnyxWebhook = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid_json", "detail": e.to_string() })),
            )
                .into_response();
        }
    };

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
    let recognized = state.recognized_senders.lock().unwrap().clone();
    let identity_level = crate::identity::resolve_sender(
        from,
        body,
        &state.config.allowed_senders,
        state.config.owner_number.as_deref(),
        state.config.owner_totp_secret.as_deref(),
        &recognized,
    );

    if identity_level == IdentityLevel::Unknown {
        tracing::warn!(from = %from, "Unknown sender — quarantined");

        // Record as recognized sender for next time (fire-and-forget)
        record_recognized_sender(&state, from).await;

        return (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "quarantined",
                "reason": "unknown_sender",
            })),
        )
            .into_response();
    }

    if identity_level == IdentityLevel::Recognized {
        tracing::info!(from = %from, "Recognized sender — quarantined (not on allowlist)");
        // Update last_seen in Datastore (fire-and-forget)
        record_recognized_sender(&state, from).await;
        return (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "quarantined",
                "reason": "recognized_not_allowed",
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

/// Verify a Telnyx Ed25519 webhook signature.
///
/// Telnyx signs: `{timestamp}|{raw_body}`
/// The public key and signature are base64-encoded.
fn verify_telnyx_signature(
    public_key_b64: &str,
    signature_b64: &str,
    timestamp: &str,
    body: &[u8],
) -> Result<(), String> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;

    let pk_bytes = engine
        .decode(public_key_b64)
        .map_err(|e| format!("invalid public key base64: {e}"))?;

    let pk_array: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| "public key must be 32 bytes".to_string())?;

    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| format!("invalid Ed25519 public key: {e}"))?;

    let sig_bytes = engine
        .decode(signature_b64)
        .map_err(|e| format!("invalid signature base64: {e}"))?;

    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| "signature must be 64 bytes".to_string())?;

    let signature = Signature::from_bytes(&sig_array);

    // Telnyx signs: timestamp|body
    let mut message = Vec::with_capacity(timestamp.len() + 1 + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.push(b'|');
    message.extend_from_slice(body);

    verifying_key
        .verify(&message, &signature)
        .map_err(|_| "Ed25519 signature verification failed".to_string())
}

/// Record a sender in the recognized senders list (Datastore + in-memory cache).
async fn record_recognized_sender(state: &crate::AppState, phone: &str) {
    // Add to in-memory list
    {
        let mut list = state.recognized_senders.lock().unwrap();
        if !list.iter().any(|s| s == phone) {
            list.push(phone.to_string());
        }
    }

    // Persist to Datastore (fire-and-forget, best-effort)
    let datastore_url = match &state.config.datastore_url {
        Some(u) => u.clone(),
        None => return,
    };

    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "collection": "_recognized_senders",
        "document": {
            "phone": phone,
            "last_seen": now,
        },
        "upsert_key": "phone",
    });

    let url = format!("{datastore_url}/upsert");
    if let Err(e) = state.http.post(&url).json(&payload).send().await {
        tracing::warn!(error = %e, phone = %phone, "Failed to persist recognized sender");
    }
}
