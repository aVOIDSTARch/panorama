use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use cloak_core::HealthResponse;

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .with_state(state)
}

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let halted = state.halted.load(Ordering::Relaxed);
    let halt_reason = state.halt_reason.read().await.clone();

    let infisical_reachable = state
        .infisical
        .health_check()
        .await
        .unwrap_or(false);

    let status = if halted {
        "halted"
    } else if !infisical_reachable {
        "degraded"
    } else {
        "healthy"
    };

    Json(HealthResponse {
        status: status.into(),
        service_id: "cloak".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        halted,
        halt_reason,
        registered_services: state.registry.count(),
        infisical_reachable,
        uptime_seconds: state.start_time.elapsed().as_secs_f64(),
    })
}
