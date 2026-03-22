//! Centralized error system for Panorama.
//!
//! Every error in the system gets a unique code (e.g. CLOAK-004, GW-008) with
//! a human-readable message, severity, retryable flag, and suggested fix.
//! Existing per-crate error enums are preserved — `From` impls convert them
//! into `PanoramaError` at the boundary.

pub mod catalog;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use catalog::{ErrorDef, Severity};
use chrono::Utc;
use uuid::Uuid;

/// A unified, coded error that any Panorama service can return.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PanoramaError {
    /// Unique instance ID for this specific error occurrence.
    pub instance_id: String,
    /// Error code from the catalog (e.g. "CLOAK-004").
    pub code: String,
    /// Human-readable message (from catalog, possibly enriched with context).
    pub message: String,
    /// Additional context about what went wrong.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Severity level.
    pub severity: Severity,
    /// Whether the caller should retry.
    pub retryable: bool,
    /// Suggested remediation.
    pub suggestion: String,
    /// Which service produced this error.
    pub service: String,
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// HTTP status code (not serialized in JSON body — used for response status).
    #[serde(skip)]
    pub status: StatusCode,
}

impl PanoramaError {
    /// Create an error from a catalog code with optional detail.
    pub fn from_code(code: &str, service: &str, detail: Option<String>) -> Self {
        let def = catalog::lookup(code);
        Self::from_def_or_fallback(code, def, service, detail)
    }

    /// Create an error directly from a static ErrorDef with optional detail.
    pub fn from_def(def: &ErrorDef, service: &str, detail: Option<String>) -> Self {
        Self {
            instance_id: Uuid::new_v4().to_string(),
            code: def.code.to_string(),
            message: def.message.to_string(),
            detail,
            severity: def.severity,
            retryable: def.retryable,
            suggestion: def.suggestion.to_string(),
            service: service.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            status: def.status,
        }
    }

    fn from_def_or_fallback(
        code: &str,
        def: Option<&ErrorDef>,
        service: &str,
        detail: Option<String>,
    ) -> Self {
        match def {
            Some(d) => Self::from_def(d, service, detail),
            None => {
                tracing::warn!("Unknown error code: {code}");
                Self {
                    instance_id: Uuid::new_v4().to_string(),
                    code: code.to_string(),
                    message: "Unknown error".to_string(),
                    detail,
                    severity: Severity::Error,
                    retryable: false,
                    suggestion: "Check service logs for details.".to_string(),
                    service: service.to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                }
            }
        }
    }
}

impl std::fmt::Display for PanoramaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(ref d) = self.detail {
            write!(f, " — {d}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PanoramaError {}

impl IntoResponse for PanoramaError {
    fn into_response(self) -> Response {
        let status = self.status;
        let body = serde_json::json!({
            "error": {
                "instance_id": self.instance_id,
                "code": self.code,
                "message": self.message,
                "detail": self.detail,
                "severity": self.severity,
                "retryable": self.retryable,
                "suggestion": self.suggestion,
                "service": self.service,
                "timestamp": self.timestamp,
            }
        });

        tracing::error!(
            error_code = %self.code,
            instance_id = %self.instance_id,
            service = %self.service,
            "{}",
            self.message,
        );

        (status, axum::Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors for common patterns
// ---------------------------------------------------------------------------

impl PanoramaError {
    pub fn internal(service: &str, detail: impl Into<String>) -> Self {
        // Pick the appropriate internal error code based on service prefix
        let code = match service {
            "cloak" => "CLOAK-013",
            "cortex" => "CTX-008",
            "gateway" => "GW-009",
            _ => "GW-009", // fallback
        };
        Self::from_code(code, service, Some(detail.into()))
    }

    pub fn unauthorized(service: &str, detail: impl Into<String>) -> Self {
        let code = match service {
            "cloak" => "CLOAK-002",
            "cortex" => "CTX-004",
            "gateway" => "GW-002",
            _ => "GW-002",
        };
        Self::from_code(code, service, Some(detail.into()))
    }
}
