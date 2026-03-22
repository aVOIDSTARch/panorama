use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use panorama_errors::PanoramaError;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum CloakError {
    #[error("Infisical unavailable")]
    InfisicalUnavailable,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Malformed token")]
    MalformedToken,

    #[error("Missing token")]
    MissingToken,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("No signing key")]
    NoSigningKey,

    #[error("Insufficient permissions: {0}")]
    InsufficientPermissions(String),

    #[error("Service not in scope: {0}")]
    ServiceNotInScope(String),

    #[error("Service not registered: {0}")]
    ServiceNotRegistered(String),

    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    #[error("Service halted: {0}")]
    Halted(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for CloakError {
    fn into_response(self) -> Response {
        let (status, error_code) = match &self {
            CloakError::InfisicalUnavailable => {
                (StatusCode::SERVICE_UNAVAILABLE, "infisical_unavailable")
            }
            CloakError::InvalidToken(_) => (StatusCode::UNAUTHORIZED, "invalid_token"),
            CloakError::MalformedToken => (StatusCode::UNAUTHORIZED, "malformed_token"),
            CloakError::MissingToken => (StatusCode::UNAUTHORIZED, "missing_token"),
            CloakError::InvalidSignature => (StatusCode::UNAUTHORIZED, "invalid_signature"),
            CloakError::NoSigningKey => (StatusCode::SERVICE_UNAVAILABLE, "no_signing_key"),
            CloakError::InsufficientPermissions(_) => {
                (StatusCode::FORBIDDEN, "insufficient_permissions")
            }
            CloakError::ServiceNotInScope(_) => {
                (StatusCode::FORBIDDEN, "service_not_in_scope")
            }
            CloakError::ServiceNotRegistered(_) => {
                (StatusCode::NOT_FOUND, "service_not_registered")
            }
            CloakError::RegistrationFailed(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "registration_failed")
            }
            CloakError::Halted(_) => (StatusCode::SERVICE_UNAVAILABLE, "service_halted"),
            CloakError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "config_error"),
            CloakError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = json!({
            "error": error_code,
            "detail": self.to_string(),
            "service": "cloak",
        });

        (status, axum::Json(body)).into_response()
    }
}

impl From<CloakError> for PanoramaError {
    fn from(err: CloakError) -> Self {
        let (code, detail) = match &err {
            CloakError::InfisicalUnavailable => ("CLOAK-001", None),
            CloakError::InvalidToken(d) => ("CLOAK-002", Some(d.clone())),
            CloakError::MalformedToken => ("CLOAK-003", None),
            CloakError::MissingToken => ("CLOAK-004", None),
            CloakError::InvalidSignature => ("CLOAK-005", None),
            CloakError::NoSigningKey => ("CLOAK-006", None),
            CloakError::InsufficientPermissions(d) => ("CLOAK-007", Some(d.clone())),
            CloakError::ServiceNotInScope(d) => ("CLOAK-008", Some(d.clone())),
            CloakError::ServiceNotRegistered(d) => ("CLOAK-009", Some(d.clone())),
            CloakError::RegistrationFailed(d) => ("CLOAK-010", Some(d.clone())),
            CloakError::Halted(d) => ("CLOAK-011", Some(d.clone())),
            CloakError::Config(d) => ("CLOAK-012", Some(d.clone())),
            CloakError::Internal(d) => ("CLOAK-013", Some(d.clone())),
        };
        PanoramaError::from_code(code, "cloak", detail)
    }
}
