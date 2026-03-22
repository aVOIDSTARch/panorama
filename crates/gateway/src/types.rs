use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Provider enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Provider {
    Anthropic,
    OpenAI,
    Mistral,
    Groq,
    Custom { name: String },
}

// ---------------------------------------------------------------------------
// Route record (§4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub route_key: String,
    pub display_name: String,
    pub provider: Provider,
    pub model_id: String,
    pub endpoint_url: String,
    pub api_key_env: String,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub cost_per_input_token_usd: f64,
    pub cost_per_output_token_usd: f64,
    pub fallback_chain: Vec<String>,
    pub health_probe_interval_secs: u64,
    pub active: bool,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Request / Response types (§17)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRequest {
    pub request_id: Uuid,
    pub route_key: String,
    pub prompt: String,
    pub caller_metadata: CallerMetadata,
    pub options: RequestOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallerMetadata {
    pub caller_id: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct SanitizedRequest {
    pub request_id: Uuid,
    pub route_key: String,
    pub prompt: String,
    pub caller_id: String,
    pub session_id: Option<String>,
    pub options: RequestOptions,
    pub inbound_hash: String,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub request_id: Uuid,
    pub raw_response: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub provider_latency_ms: u64,
    pub route_key: String,
    pub fallback_attempt: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundResponse {
    pub request_id: Uuid,
    pub status: ResponseStatus,
    pub route_key_used: String,
    pub fallback_triggered: bool,
    pub response: Option<String>,
    pub usage: Option<UsageSummary>,
    pub error: Option<GatewayErrorBody>,
    pub latency_ms: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    SanitizationRejected,
    RateLimited,
    Deduplicated,
    BudgetExceeded,
    RouteNotFound,
    RouteUnhealthy,
    ProviderError,
    AllFallbacksExhausted,
    GatewayHalted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayErrorBody {
    pub code: u16,
    pub kind: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub estimated_cost_usd: f64,
}

// ---------------------------------------------------------------------------
// Cost accounting (§7)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecord {
    pub request_id: Uuid,
    pub caller_id: String,
    pub route_key: String,
    pub route_key_used: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub estimated_cost_usd: f64,
    pub fallback_triggered: bool,
    pub fallback_attempt: u8,
    pub timestamp: DateTime<Utc>,
    pub outcome: String,
}

// ---------------------------------------------------------------------------
// Caller identity (§13)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallerIdentity {
    pub caller_id: String,
    pub token_hash: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub allowed_routes: Vec<String>,
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Alerts (§9)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_id: Uuid,
    pub level: AlertLevel,
    pub source: AlertSource,
    pub request_id: Option<Uuid>,
    pub route_key: Option<String>,
    pub caller_id: Option<String>,
    pub message: String,
    pub detail: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub dispatched_to: Vec<AlertDestination>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum AlertLevel {
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertSource {
    InboundSanitizer,
    OutboundSanitizer,
    RouteDispatcher,
    FallbackChain,
    HealthProber,
    KillSwitch,
    CostAccountant,
    RateLimiter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub enum AlertDestination {
    Log,
    Webhook { url: String },
    Sms { to: String },
}

// ---------------------------------------------------------------------------
// Sanitization (§5)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SanitizationError {
    pub kind: SanitizationErrorKind,
    pub field: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SanitizationErrorKind {
    SchemaViolation,
    ContentViolation,
    EncodingError,
    SizeLimitExceeded,
    InjectionPattern,
    MalformedJson,
}

// ---------------------------------------------------------------------------
// Kill switch (§10)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum KillSwitchState {
    Operational,
    Drain,
    Halted,
}

// ---------------------------------------------------------------------------
// Health status (§12)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}
