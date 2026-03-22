use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub budgets: BudgetsConfig,
    pub health_probing: HealthProbingConfig,
    pub deduplication: DeduplicationConfig,
    pub kill_switch: KillSwitchConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
    #[serde(default)]
    pub rate_limits: RateLimitsConfig,
    #[serde(default)]
    pub sanitizer: SanitizerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_admin_port")]
    pub admin_port: u16,
    #[serde(default)]
    pub tls_cert_path: String,
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_drain_timeout")]
    pub drain_timeout_secs: u64,
    #[serde(default)]
    pub admin_token_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_operational_db")]
    pub operational_db_path: String,
    #[serde(default = "default_audit_db")]
    pub audit_db_path: String,
    #[serde(default = "default_route_store")]
    pub route_store_path: String,
    #[serde(default = "default_retention_days")]
    pub operational_retention_days: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BudgetsConfig {
    #[serde(default = "default_global_daily")]
    pub global_daily_usd: f64,
    #[serde(default = "default_per_caller_daily")]
    pub per_caller_daily_usd: f64,
    #[serde(default = "default_per_route_daily")]
    pub per_route_daily_usd: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthProbingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_probe_interval")]
    pub default_interval_secs: u64,
    #[serde(default = "default_probe_timeout")]
    pub probe_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeduplicationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_dedup_window")]
    pub window_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KillSwitchConfig {
    #[serde(default = "default_consecutive_criticals")]
    pub auto_drain_on_consecutive_criticals: u32,
    #[serde(default = "default_true")]
    pub auto_halt_on_credential_scrub: bool,
    #[serde(default)]
    pub auto_drain_on_global_budget_hit: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AlertsConfig {
    #[serde(default)]
    pub destinations: AlertDestinationsConfig,
    #[serde(default)]
    pub routing: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub source_overrides: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub suppression: AlertSuppressionConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AlertDestinationsConfig {
    #[serde(default)]
    pub telnyx: Option<TelnyxConfig>,
    #[serde(default)]
    pub webhook: Option<WebhookConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelnyxConfig {
    pub enabled: bool,
    pub from_number: String,
    pub to_numbers: Vec<String>,
    pub api_key_env: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookConfig {
    pub enabled: bool,
    pub url: String,
    pub secret_env: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AlertSuppressionConfig {
    #[serde(default = "default_sms_cooldown")]
    pub sms_cooldown_secs: u64,
    #[serde(default = "default_max_sms_per_hour")]
    pub max_sms_per_hour: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RateLimitsConfig {
    #[serde(default)]
    pub defaults: RateLimitDefaults,
    #[serde(default)]
    pub callers: HashMap<String, RateLimitOverride>,
    #[serde(default)]
    pub routes: HashMap<String, RateLimitOverride>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitDefaults {
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,
    #[serde(default = "default_rph")]
    pub requests_per_hour: u32,
}

impl Default for RateLimitDefaults {
    fn default() -> Self {
        Self {
            requests_per_minute: default_rpm(),
            requests_per_hour: default_rph(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitOverride {
    pub requests_per_minute: Option<u32>,
    pub requests_per_hour: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SanitizerConfig {
    #[serde(default)]
    pub injection_patterns: Vec<String>,
    #[serde(default)]
    pub credential_patterns: Vec<String>,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

fn default_host() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 8800 }
fn default_admin_port() -> u16 { 8801 }
fn default_request_timeout() -> u64 { 30 }
fn default_drain_timeout() -> u64 { 60 }
fn default_operational_db() -> String { "data/gateway_logs.db".to_string() }
fn default_audit_db() -> String { "data/gateway_audit.db".to_string() }
fn default_route_store() -> String { "data/gateway_routes.db".to_string() }
fn default_retention_days() -> u32 { 90 }
fn default_global_daily() -> f64 { 20.0 }
fn default_per_caller_daily() -> f64 { 5.0 }
fn default_per_route_daily() -> f64 { 10.0 }
fn default_probe_interval() -> u64 { 300 }
fn default_probe_timeout() -> u64 { 10 }
fn default_dedup_window() -> u64 { 30 }
fn default_consecutive_criticals() -> u32 { 5 }
fn default_true() -> bool { true }
fn default_sms_cooldown() -> u64 { 300 }
fn default_max_sms_per_hour() -> u32 { 10 }
fn default_rpm() -> u32 { 60 }
fn default_rph() -> u32 { 500 }

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

impl GatewayConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read config at {}: {e}", path.display()))?;
        let config: GatewayConfig = toml::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("failed to parse config: {e}"))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let mut errors = Vec::new();

        if self.server.port == 0 {
            errors.push("server.port must be > 0".to_string());
        }
        if self.server.admin_port == 0 {
            errors.push("server.admin_port must be > 0".to_string());
        }
        if self.server.port == self.server.admin_port {
            errors.push("server.port and server.admin_port must differ".to_string());
        }
        if self.budgets.global_daily_usd < 0.0 {
            errors.push("budgets.global_daily_usd must be >= 0".to_string());
        }
        if self.budgets.per_caller_daily_usd < 0.0 {
            errors.push("budgets.per_caller_daily_usd must be >= 0".to_string());
        }
        if self.budgets.per_route_daily_usd < 0.0 {
            errors.push("budgets.per_route_daily_usd must be >= 0".to_string());
        }
        if self.server.request_timeout_secs == 0 {
            errors.push("server.request_timeout_secs must be > 0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "config validation failed:\n  - {}",
                errors.join("\n  - ")
            ))
        }
    }
}
