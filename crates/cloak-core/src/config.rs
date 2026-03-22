use std::env;

use crate::error::CloakError;

#[derive(Debug, Clone)]
pub struct CloakConfig {
    pub port: u16,
    pub infisical_url: String,
    pub infisical_token: String,
    pub infisical_project: String,
    pub infisical_env: String,
    pub secret_cache_ttl_secs: u64,
    pub log_level: String,
    pub tailscale_interface: String,
    pub admin_password_hash: Option<String>,
}

impl CloakConfig {
    pub fn from_env() -> Result<Self, CloakError> {
        let config = Self {
            port: env::var("CLOAK_PORT")
                .unwrap_or_else(|_| "8300".into())
                .parse()
                .map_err(|e| CloakError::Config(format!("Invalid CLOAK_PORT: {e}")))?,
            infisical_url: env::var("CLOAK_INFISICAL_URL")
                .map_err(|_| CloakError::Config("CLOAK_INFISICAL_URL is required".into()))?,
            infisical_token: env::var("CLOAK_INFISICAL_TOKEN")
                .map_err(|_| CloakError::Config("CLOAK_INFISICAL_TOKEN is required".into()))?,
            infisical_project: env::var("CLOAK_INFISICAL_PROJECT")
                .map_err(|_| CloakError::Config("CLOAK_INFISICAL_PROJECT is required".into()))?,
            infisical_env: env::var("CLOAK_INFISICAL_ENV")
                .unwrap_or_else(|_| "production".into()),
            secret_cache_ttl_secs: env::var("CLOAK_SECRET_CACHE_TTL")
                .unwrap_or_else(|_| "30".into())
                .parse()
                .map_err(|e| CloakError::Config(format!("Invalid CLOAK_SECRET_CACHE_TTL: {e}")))?,
            log_level: env::var("CLOAK_LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            tailscale_interface: env::var("CLOAK_TAILSCALE_INTERFACE")
                .unwrap_or_else(|_| "tailscale0".into()),
            admin_password_hash: env::var("CLOAK_ADMIN_PASSWORD_HASH").ok(),
        };

        Ok(config)
    }
}
