use axum::extract::State;
use axum::routing::{delete, get, post};
use axum::{Json, Router};

use cloak_core::CloakError;

use crate::model::{PermissionEntry, PermissionStore};

pub fn routes() -> Router<PermissionStore> {
    Router::new()
        .route("/cloak/admin/permissions", get(list_permissions))
        .route("/cloak/admin/permissions", post(add_permission))
        .route("/cloak/admin/permissions", delete(remove_permission))
}

async fn list_permissions(
    State(store): State<PermissionStore>,
) -> Json<Vec<PermissionEntry>> {
    Json(store.list().await)
}

async fn add_permission(
    State(store): State<PermissionStore>,
    Json(entry): Json<PermissionEntry>,
) -> Result<Json<serde_json::Value>, CloakError> {
    store.add(entry).await;
    Ok(Json(serde_json::json!({"status": "added"})))
}

#[derive(serde::Deserialize)]
struct RemoveRequest {
    identity_pattern: String,
    service: String,
}

async fn remove_permission(
    State(store): State<PermissionStore>,
    Json(req): Json<RemoveRequest>,
) -> Result<Json<serde_json::Value>, CloakError> {
    let removed = store.remove(&req.identity_pattern, &req.service).await;
    Ok(Json(serde_json::json!({"removed": removed})))
}
