use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::job::JobPriority;
use crate::state::AppState;

/// Submit a job request to the Hub.
///
/// The Hub is the external-facing tier that receives requests,
/// routes them through cascade logic, and dispatches to the Orchestrator.
#[derive(Deserialize)]
pub struct SubmitJobRequest {
    pub description: String,
    pub requester: String,
    pub priority: Option<JobPriority>,
}

pub async fn submit_job(
    State(state): State<AppState>,
    Json(req): Json<SubmitJobRequest>,
) -> impl IntoResponse {
    let priority = req.priority.unwrap_or(JobPriority::Normal);

    match state.orchestrator.submit_job(&req.description, &req.requester, priority).await {
        Ok(job) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "job_id": job.job_id,
                "status": job.status,
                "tasks": job.tasks.len(),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Get orchestrator status and pool stats.
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.orchestrator.pool_stats();

    Json(json!({
        "status": "operational",
        "service": "wheelhouse",
        "pool": {
            "total": stats.total,
            "idle": stats.idle,
            "active": stats.active,
            "retiring": stats.retiring,
        },
    }))
}

/// List all agents in the pool.
pub async fn agents(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.orchestrator.agents_list();
    Json(json!({
        "agents": agents,
    }))
}

/// Health check.
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let halted = state.cloak.is_halted().await;
    let code = if halted {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };

    (
        code,
        Json(json!({
            "status": if halted { "halted" } else { "ok" },
            "service": "wheelhouse",
            "halted": halted,
        })),
    )
}
