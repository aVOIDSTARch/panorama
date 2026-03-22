use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- Operation Class ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationClass {
    Read,
    Write,
    Admin,
}

impl OperationClass {
    pub fn level(&self) -> u8 {
        match self {
            Self::Read => 0,
            Self::Write => 1,
            Self::Admin => 2,
        }
    }

    pub fn satisfies(&self, required: &OperationClass) -> bool {
        self.level() >= required.level()
    }
}

// --- Token Claims (full token payload, matches cortex-core::Token) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub job_id: String,
    pub agent_class: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub services: Vec<ServiceScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceScope {
    pub service: String,
    pub operation_class: OperationClass,
    pub resources: Vec<String>,
}

// --- Service Registration ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationRequest {
    pub service_id: String,
    pub service_type: String,
    pub version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResponse {
    pub session_id: String,
    pub signing_key: String, // base64-encoded
    pub halt_stream_url: String,
}

// --- Token Validation ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    pub token: String,
    pub service: String,
    pub operation: String,
    pub resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResponse {
    pub allowed: bool,
    pub reason: String,
}

// --- Token Issuance ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenIssueRequest {
    pub job_id: String,
    pub agent_class: String,
    pub ttl_seconds: u64,
    pub services: Vec<ServiceScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenIssueResponse {
    pub token: String,
    pub scope: TokenClaims,
}

// --- SSE Halt Events ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaltEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "halt" | "key_rotation"
    pub service_id: Option<String>,
    pub reason: Option<String>,
    pub new_key: Option<String>, // base64-encoded, for key_rotation
}

// --- Health ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service_id: String,
    pub version: String,
    pub halted: bool,
    pub halt_reason: Option<String>,
    pub registered_services: usize,
    pub infisical_reachable: bool,
    pub uptime_seconds: f64,
}
