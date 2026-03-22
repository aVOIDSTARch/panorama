use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::warn;

use crate::state::AppState;

/// Proxy handler: /{service_name}/{rest_of_path}
///
/// Forwards the request to the configured base_url for the service,
/// preserving path, query, method, headers, and body.
pub async fn proxy_request(
    State(state): State<AppState>,
    Path((service_name, rest)): Path<(String, String)>,
    request: Request,
) -> Response {
    // Look up service in manifest
    let svc_config = match state.config.manifest.services.get(&service_name) {
        Some(cfg) => cfg.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "service_not_found",
                    "detail": format!("No service '{service_name}' in manifest"),
                    "service": "cortex",
                })),
            )
                .into_response();
        }
    };

    // Check failure state
    if let Some(fs) = state.service_failure_state(&service_name).await {
        if !fs.allows_requests() {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": "service_unavailable",
                    "detail": format!("Service '{service_name}' is in {fs:?} state"),
                    "service": "cortex",
                    "failure_state": format!("{fs:?}"),
                })),
            )
                .into_response();
        }
    }

    // Build target URL
    let target_url = format!("{}/{}", svc_config.base_url.trim_end_matches('/'), rest);
    let query = request.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let full_url = format!("{target_url}{query}");

    // Forward the request
    let method = request.method().clone();
    let headers = request.headers().clone();
    let body = request.into_body();

    let mut req_builder = state.http.request(method, &full_url);

    // Forward relevant headers (skip host, transfer-encoding)
    for (key, value) in headers.iter() {
        let name = key.as_str();
        if name != "host" && name != "transfer-encoding" {
            req_builder = req_builder.header(key, value);
        }
    }

    // Forward body
    let body_bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "body_read_error", "detail": e.to_string() })),
            )
                .into_response();
        }
    };

    if !body_bytes.is_empty() {
        req_builder = req_builder.body(body_bytes.to_vec());
    }

    // Execute
    let timeout = std::time::Duration::from_millis(svc_config.timeout_ms);
    match tokio::time::timeout(timeout, req_builder.send()).await {
        Ok(Ok(resp)) => {
            // Record success
            state.record_health_success(&service_name).await;

            // Convert reqwest::Response -> axum::Response
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let headers = resp.headers().clone();
            let body_bytes = resp.bytes().await.unwrap_or_default();

            let mut response = (status, body_bytes.to_vec()).into_response();
            for (key, value) in headers.iter() {
                response.headers_mut().insert(key, value.clone());
            }
            response
        }
        Ok(Err(e)) => {
            state.record_health_failure(&service_name).await;
            warn!(service = %service_name, error = %e, "Proxy request failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "proxy_error",
                    "detail": format!("Failed to reach {service_name}: {e}"),
                    "service": "cortex",
                })),
            )
                .into_response()
        }
        Err(_) => {
            state.record_health_failure(&service_name).await;
            warn!(service = %service_name, "Proxy request timed out");
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({
                    "error": "timeout",
                    "detail": format!("Request to {service_name} timed out after {}ms", svc_config.timeout_ms),
                    "service": "cortex",
                })),
            )
                .into_response()
        }
    }
}
