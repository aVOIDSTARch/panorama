use std::env;

use crate::error::CloakError;

#[derive(Debug, Clone)]
pub enum InfisicalAuth {
    /// Universal Auth: client ID + secret, token obtained at runtime
    UniversalAuth {
        client_id: String,
        client_secret: String,
    },
    /// Static bearer token (legacy / testing)
    StaticToken(String),
}

#[derive(Debug, Clone)]
pub struct CloakConfig {
    pub port: u16,
    pub infisical_url: String,
    pub infisical_auth: InfisicalAuth,
    pub infisical_project: String,
    pub infisical_env: String,
    pub secret_cache_ttl_secs: u64,
    pub log_level: String,
    pub tailscale_interface: String,
    pub admin_password_hash: Option<String>,
}

impl CloakConfig {
    pub fn from_env() -> Result<Self, CloakError> {
        // Support Universal Auth (client_id + client_secret) or legacy static token
        let infisical_auth =
            match (env::var("CLOAK_INFISICAL_CLIENT_ID"), env::var("CLOAK_INFISICAL_CLIENT_SECRET")) {
                (Ok(id), Ok(secret)) if !id.is_empty() && !secret.is_empty() => {
                    InfisicalAuth::UniversalAuth {
                        client_id: id,
                        client_secret: secret,
                    }
                }
                _ => {
                    let token = env::var("CLOAK_INFISICAL_TOKEN")
                        .map_err(|_| CloakError::Config(
                            "Either CLOAK_INFISICAL_CLIENT_ID + CLOAK_INFISICAL_CLIENT_SECRET \
                             or CLOAK_INFISICAL_TOKEN is required".into()
                        ))?;
                    InfisicalAuth::StaticToken(token)
                }
            };

        let config = Self {
            port: env::var("CLOAK_PORT")
                .unwrap_or_else(|_| "8300".into())
                .parse()
                .map_err(|e| CloakError::Config(format!("Invalid CLOAK_PORT: {e}")))?,
            infisical_url: env::var("CLOAK_INFISICAL_URL")
                .map_err(|_| CloakError::Config("CLOAK_INFISICAL_URL is required".into()))?,
            infisical_auth,
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

    /// Create a config suitable for integration tests — dummy Infisical values,
    /// no env vars needed.
    pub fn for_testing(port: u16) -> Self {
        Self {
            port,
            infisical_url: "http://127.0.0.1:19999".into(),
            infisical_auth: InfisicalAuth::StaticToken("test-token".into()),
            infisical_project: "test-project".into(),
            infisical_env: "test".into(),
            secret_cache_ttl_secs: 3600,
            log_level: "warn".into(),
            tailscale_interface: "lo0".into(),
            admin_password_hash: None,
        }
    }
}
