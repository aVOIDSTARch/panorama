use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum GatewayApiError {
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("rate limited")]
    RateLimited { retry_after_secs: u64 },

    #[error("duplicate request: original {original_request_id}")]
    Conflict { original_request_id: String },

    #[error("budget exceeded: {0}")]
    PaymentRequired(String),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("bad gateway: {0}")]
    BadGateway(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for GatewayApiError {
    fn into_response(self) -> Response {
        let (status, kind, message, retryable) = match &self {
            GatewayApiError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", msg.clone(), false)
            }
            GatewayApiError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, "unauthorized", msg.clone(), false)
            }
            GatewayApiError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, "not_found", msg.clone(), false)
            }
            GatewayApiError::RateLimited { retry_after_secs } => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                format!("retry after {retry_after_secs}s"),
                true,
            ),
            GatewayApiError::Conflict {
                original_request_id,
            } => (
                StatusCode::CONFLICT,
                "duplicate_request",
                format!("duplicate of {original_request_id}"),
                false,
            ),
            GatewayApiError::PaymentRequired(msg) => (
                StatusCode::PAYMENT_REQUIRED,
                "budget_exceeded",
                msg.clone(),
                false,
            ),
            GatewayApiError::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                msg.clone(),
                true,
            ),
            GatewayApiError::BadGateway(msg) => {
                (StatusCode::BAD_GATEWAY, "bad_gateway", msg.clone(), true)
            }
            GatewayApiError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg.clone(),
                false,
            ),
        };

        let body = json!({
            "error": {
                "code": status.as_u16(),
                "kind": kind,
                "message": message,
                "retryable": retryable,
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

impl From<anyhow::Error> for GatewayApiError {
    fn from(err: anyhow::Error) -> Self {
        GatewayApiError::Internal(err.to_string())
    }
}

impl From<rusqlite::Error> for GatewayApiError {
    fn from(err: rusqlite::Error) -> Self {
        GatewayApiError::Internal(format!("database error: {err}"))
    }
}
