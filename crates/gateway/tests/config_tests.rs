use gateway::config::GatewayConfig;
use std::io::Write;

fn write_temp_config(content: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

const MINIMAL_VALID_CONFIG: &str = r#"
[server]
port = 8800
admin_port = 8801

[database]
operational_db_path = "data/logs.db"
audit_db_path = "data/audit.db"
route_store_path = "data/routes.db"

[budgets]
global_daily_usd = 20.0
per_caller_daily_usd = 5.0
per_route_daily_usd = 10.0

[health_probing]
enabled = true

[deduplication]
enabled = true
window_secs = 30

[kill_switch]
auto_drain_on_consecutive_criticals = 5
auto_halt_on_credential_scrub = true
"#;

#[test]
fn load_minimal_valid_config() {
    let f = write_temp_config(MINIMAL_VALID_CONFIG);
    let config = GatewayConfig::load(f.path()).unwrap();
    assert_eq!(config.server.port, 8800);
    assert_eq!(config.server.admin_port, 8801);
    assert_eq!(config.budgets.global_daily_usd, 20.0);
    assert!(config.kill_switch.auto_halt_on_credential_scrub);
}

#[test]
fn load_with_defaults() {
    let f = write_temp_config(MINIMAL_VALID_CONFIG);
    let config = GatewayConfig::load(f.path()).unwrap();
    // Check defaulted fields
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.request_timeout_secs, 30);
    assert_eq!(config.server.drain_timeout_secs, 60);
    assert_eq!(config.rate_limits.defaults.requests_per_minute, 60);
    assert_eq!(config.rate_limits.defaults.requests_per_hour, 500);
}

#[test]
fn validation_same_ports_fails() {
    let toml = MINIMAL_VALID_CONFIG.replace("admin_port = 8801", "admin_port = 8800");
    let f = write_temp_config(&toml);
    let result = GatewayConfig::load(f.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("port") && err.contains("differ"));
}

#[test]
fn validation_zero_port_fails() {
    let toml = MINIMAL_VALID_CONFIG.replace("port = 8800", "port = 0");
    let f = write_temp_config(&toml);
    let result = GatewayConfig::load(f.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("port") && err.contains("> 0"));
}

#[test]
fn validation_negative_budget_fails() {
    let toml = MINIMAL_VALID_CONFIG.replace("global_daily_usd = 20.0", "global_daily_usd = -1.0");
    let f = write_temp_config(&toml);
    let result = GatewayConfig::load(f.path());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("global_daily_usd") && err.contains(">= 0"));
}

#[test]
fn validation_zero_timeout_fails() {
    let toml = format!(
        "{}\n[server]\nrequest_timeout_secs = 0\nport = 8800\nadmin_port = 8801\n",
        "[database]\noperational_db_path = \"x\"\naudit_db_path = \"x\"\nroute_store_path = \"x\"\n\
         [budgets]\n[health_probing]\n[deduplication]\n[kill_switch]"
    );
    let f = write_temp_config(&toml);
    let result = GatewayConfig::load(f.path());
    assert!(result.is_err());
}

#[test]
fn nonexistent_file_fails() {
    let result = GatewayConfig::load(std::path::Path::new("/nonexistent/gateway.toml"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("failed to read"));
}

#[test]
fn invalid_toml_fails() {
    let f = write_temp_config("this is not valid toml {{{");
    let result = GatewayConfig::load(f.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("failed to parse"));
}

#[test]
fn config_with_sanitizer_patterns() {
    let toml = format!(
        r#"{}

[sanitizer]
injection_patterns = ["(?i)ignore.*instructions"]
credential_patterns = ["sk-[a-zA-Z0-9]{{20,}}"]
"#,
        MINIMAL_VALID_CONFIG
    );
    let f = write_temp_config(&toml);
    let config = GatewayConfig::load(f.path()).unwrap();
    assert_eq!(config.sanitizer.injection_patterns.len(), 1);
    assert_eq!(config.sanitizer.credential_patterns.len(), 1);
}

#[test]
fn config_with_rate_limit_overrides() {
    let toml = format!(
        r#"{}

[rate_limits.defaults]
requests_per_minute = 30
requests_per_hour = 200

[rate_limits.callers.vip]
requests_per_minute = 120
"#,
        MINIMAL_VALID_CONFIG
    );
    let f = write_temp_config(&toml);
    let config = GatewayConfig::load(f.path()).unwrap();
    assert_eq!(config.rate_limits.defaults.requests_per_minute, 30);
    assert_eq!(
        config.rate_limits.callers.get("vip").unwrap().requests_per_minute,
        Some(120)
    );
}
