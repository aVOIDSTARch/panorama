use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Schema routes
// ---------------------------------------------------------------------------

pub async fn list_tables(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_tables() {
        Ok(tables) => {
            let body: Vec<serde_json::Value> = tables
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "created_at": t.created_at,
                        "schema": t.schema_json.as_deref().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "tables": body }))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateTableRequest {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

#[derive(Deserialize, Serialize)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: String,
}

pub async fn create_table(
    State(state): State<AppState>,
    Json(req): Json<CreateTableRequest>,
) -> impl IntoResponse {
    let cols: Vec<(String, String)> = req
        .columns
        .iter()
        .map(|c| (c.name.clone(), c.col_type.clone()))
        .collect();
    match state.db.create_table(&req.name, &cols) {
        Ok(()) => (StatusCode::CREATED, Json(json!({ "created": req.name }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Object CRUD routes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ListParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn list_objects(
    State(state): State<AppState>,
    Path(table): Path<String>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    match state.db.list_objects(&table, limit, offset) {
        Ok(rows) => (StatusCode::OK, Json(json!({ "rows": rows, "count": rows.len() }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_object(
    State(state): State<AppState>,
    Path((table, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.get_object(&table, &id) {
        Ok(Some(row)) => (StatusCode::OK, Json(row)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found", "table": table, "id": id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn insert_object(
    State(state): State<AppState>,
    Path(table): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> impl IntoResponse {
    match state.db.insert_object(&table, &data) {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_object(
    State(state): State<AppState>,
    Path((table, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.delete_object(&table, &id) {
        Ok(true) => (StatusCode::OK, Json(json!({ "deleted": true }))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Query routes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct QueryRequest {
    pub sql: String,
    pub params: Option<Vec<String>>,
}

pub async fn execute_query(
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> impl IntoResponse {
    let params = req.params.unwrap_or_default();
    match state.db.execute_query(&req.sql, &params) {
        Ok(rows) => (StatusCode::OK, Json(json!({ "rows": rows, "count": rows.len() }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Blob routes
// ---------------------------------------------------------------------------

pub async fn upload_blob(
    State(state): State<AppState>,
    Path(namespace): Path<String>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let extension = "bin"; // Default extension; caller can specify via query param in future
    match state.blobs.store(&namespace, &body, extension) {
        Ok(record) => (
            StatusCode::CREATED,
            Json(json!({
                "id": record.id,
                "path": record.path,
                "sha256": record.sha256,
                "size_bytes": record.size_bytes,
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

pub async fn get_blob(
    State(state): State<AppState>,
    Path((namespace, blob_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let relative_path = format!("{namespace}/{blob_id}");
    match state.blobs.read(&relative_path) {
        Ok(data) => (
            StatusCode::OK,
            [("content-type", "application/octet-stream")],
            data,
        )
            .into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_blob(
    State(state): State<AppState>,
    Path((namespace, blob_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let relative_path = format!("{namespace}/{blob_id}");
    match state.blobs.delete(&relative_path) {
        Ok(true) => (StatusCode::OK, Json(json!({ "deleted": true }))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

pub async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let halted = state.cloak.is_halted().await;
    let status = if halted { "halted" } else { "ok" };
    let code = if halted {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };

    (
        code,
        Json(json!({
            "status": status,
            "service": "datastore",
            "halted": halted,
            "halt_reason": state.cloak.halt_reason().await,
        })),
    )
}
