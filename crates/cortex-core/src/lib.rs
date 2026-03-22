use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export shared types from cloak-core so downstream crates don't need
// to depend on cloak-core directly for these.
pub use cloak_core::{OperationClass, ServiceScope, TokenClaims};

/// Service manifest — loaded from TOML at startup.
/// Maps service names to their configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceManifest {
    pub services: HashMap<String, ServiceConfig>,
}

/// Configuration for a single downstream service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub base_url: String,
    pub health_path: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_queue_ttl")]
    pub queue_ttl_s: u64,
}

fn default_timeout() -> u64 {
    5000
}
fn default_queue_ttl() -> u64 {
    30
}

/// Per-service health state tracked by Cortex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthState {
    Healthy,
    Unhealthy,
}

/// Failure state machine for downstream services.
///
/// Transitions: Healthy -> Queuing -> Degraded -> PartialFail -> HardFail
/// Reset to Healthy on successful health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FailureState {
    Healthy,
    Queuing,
    Degraded,
    PartialFail,
    HardFail,
}

impl FailureState {
    /// Advance to the next failure state.
    pub fn escalate(&self) -> Self {
        match self {
            Self::Healthy => Self::Queuing,
            Self::Queuing => Self::Degraded,
            Self::Degraded => Self::PartialFail,
            Self::PartialFail => Self::HardFail,
            Self::HardFail => Self::HardFail,
        }
    }

    /// Whether requests should still be attempted.
    pub fn allows_requests(&self) -> bool {
        matches!(self, Self::Healthy | Self::Queuing)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CortexError {
    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },
    #[error("Service not found: {service}")]
    ServiceNotFound { service: String },
    #[error("Auth service unavailable")]
    AuthServiceUnavailable,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    #[error("Request timeout")]
    Timeout,
    #[error("Proxy error: {0}")]
    ProxyError(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ServiceManifest {
    /// Load manifest from a TOML string.
    pub fn from_toml(content: &str) -> Result<Self, CortexError> {
        toml::from_str(content)
            .map_err(|e| CortexError::Internal(format!("Failed to parse manifest: {e}")))
    }

    /// Load manifest from a TOML file.
    pub fn from_file(path: &str) -> Result<Self, CortexError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CortexError::Internal(format!("Failed to read manifest: {e}")))?;
        Self::from_toml(&content)
    }
}
