use std::sync::atomic::Ordering;

use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};

use cloak_core::CloakError;
use cloak_registry::sse;

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/cloak/admin/halt", post(halt_all))
        .route("/cloak/admin/resume", post(resume))
        .route("/cloak/admin/halt/:service_id", post(halt_service))
        .with_state(state)
}

/// YubiKey Level 1 halt — sets global HALTED flag, broadcasts halt to all services.
async fn halt_all(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, CloakError> {
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("operator");

    state.halted.store(true, Ordering::SeqCst);
    *state.halt_reason.write().await = Some(reason.to_string());

    // Broadcast halt to all registered services
    let event = sse::halt_event(None, reason);
    state.registry.broadcast(event);

    tracing::warn!("HALT ALL: {reason}");

    Ok(Json(serde_json::json!({
        "status": "halted",
        "reason": reason,
    })))
}

/// Resume from halt state.
async fn resume(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, CloakError> {
    state.halted.store(false, Ordering::SeqCst);
    *state.halt_reason.write().await = None;

    tracing::info!("RESUMED from halt");

    Ok(Json(serde_json::json!({
        "status": "resumed",
    })))
}

/// Per-service halt — sends halt event to a specific service's SSE channel.
async fn halt_service(
    State(state): State<AppState>,
    Path(service_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, CloakError> {
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("operator");

    // Check registration first — send_to returns false both when the service
    // isn't registered AND when no SSE receivers are active (no listeners yet).
    if !state.registry.is_registered(&service_id) {
        return Err(CloakError::ServiceNotRegistered(service_id));
    }

    let event = sse::halt_event(Some(&service_id), reason);
    state.registry.send_to(&service_id, event);
    // send_to may return false if no SSE listener is connected — that's fine,
    // the halt was still recorded at the Cloak level.

    tracing::warn!("HALT service {service_id}: {reason}");

    Ok(Json(serde_json::json!({
        "status": "halted",
        "service_id": service_id,
        "reason": reason,
    })))
}
