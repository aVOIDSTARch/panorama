use std::sync::Arc;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use cloak_core::{CloakError, InfisicalAuth};

/// The sole HTTP client that talks to the self-hosted Infisical instance.
/// No other code in the system should communicate with Infisical directly.
#[derive(Debug, Clone)]
pub struct InfisicalClient {
    http: Client,
    base_url: String,
    auth_token: Arc<RwLock<String>>,
    auth_config: Option<UniversalAuthConfig>,
    project_id: String,
    environment: String,
}

#[derive(Debug, Clone)]
struct UniversalAuthConfig {
    client_id: String,
    client_secret: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniversalAuthResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfisicalSecret {
    pub key: String,
    pub value: String,
    pub version: u64,
}

#[derive(Debug, Deserialize)]
struct SecretsResponse {
    secrets: Vec<InfisicalSecretRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfisicalSecretRaw {
    secret_key: String,
    secret_value: String,
    version: u64,
}

#[derive(Debug, Serialize)]
struct TokenValidatePayload {
    token: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenValidateResponse {
    pub valid: bool,
    #[serde(default)]
    pub scope: Option<serde_json::Value>,
}

impl InfisicalClient {
    /// Create a new client. For Universal Auth, call `authenticate()` after construction
    /// to obtain the initial access token.
    pub fn new(
        base_url: String,
        auth: InfisicalAuth,
        project_id: String,
        environment: String,
    ) -> Self {
        let (token, auth_config) = match auth {
            InfisicalAuth::UniversalAuth { client_id, client_secret } => (
                String::new(),
                Some(UniversalAuthConfig { client_id, client_secret }),
            ),
            InfisicalAuth::StaticToken(token) => (token, None),
        };

        Self {
            http: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: Arc::new(RwLock::new(token)),
            auth_config,
            project_id,
            environment,
        }
    }

    /// Authenticate via Universal Auth (client ID + secret → access token).
    /// No-op for static token auth.
    pub async fn authenticate(&self) -> Result<(), CloakError> {
        let config = match &self.auth_config {
            Some(c) => c,
            None => return Ok(()), // static token, nothing to do
        };

        let url = format!("{}/api/v1/auth/universal-auth/login", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "clientId": config.client_id,
                "clientSecret": config.client_secret,
            }))
            .send()
            .await
            .map_err(|e| {
                error!("Universal Auth login failed: {e}");
                CloakError::InfisicalUnavailable
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            error!("Universal Auth returned {status}: {body}");
            return Err(CloakError::InfisicalUnavailable);
        }

        let data: UniversalAuthResponse = resp.json().await.map_err(|e| {
            error!("Failed to parse Universal Auth response: {e}");
            CloakError::Internal(format!("Universal Auth parse error: {e}"))
        })?;

        info!(
            expires_in_secs = data.expires_in,
            "Authenticated with Infisical via Universal Auth"
        );
        *self.auth_token.write().await = data.access_token;
        Ok(())
    }

    async fn bearer_token(&self) -> String {
        self.auth_token.read().await.clone()
    }

    /// Check that Infisical is reachable.
    pub async fn health_check(&self) -> Result<bool, CloakError> {
        let resp = self
            .http
            .get(format!("{}/api/status", self.base_url))
            .send()
            .await
            .map_err(|_| CloakError::InfisicalUnavailable)?;

        Ok(resp.status().is_success())
    }

    /// Fetch all secrets for the configured project/environment.
    pub async fn fetch_secrets(&self) -> Result<Vec<InfisicalSecret>, CloakError> {
        let url = format!(
            "{}/api/v3/secrets/raw?workspaceId={}&environment={}",
            self.base_url, self.project_id, self.environment
        );

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.bearer_token().await))
            .send()
            .await
            .map_err(|e| {
                error!("Infisical fetch_secrets failed: {e}");
                CloakError::InfisicalUnavailable
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            error!("Infisical returned {status}: {body}");
            return Err(CloakError::InfisicalUnavailable);
        }

        let data: SecretsResponse = resp.json().await.map_err(|e| {
            error!("Failed to parse Infisical response: {e}");
            CloakError::Internal(format!("Infisical parse error: {e}"))
        })?;

        let secrets: Vec<InfisicalSecret> = data
            .secrets
            .into_iter()
            .map(|s| InfisicalSecret {
                key: s.secret_key,
                value: s.secret_value,
                version: s.version,
            })
            .collect();

        info!("Fetched {} secrets from Infisical", secrets.len());
        Ok(secrets)
    }

    /// Fetch a single secret by key.
    pub async fn fetch_secret(&self, key: &str) -> Result<InfisicalSecret, CloakError> {
        let url = format!(
            "{}/api/v3/secrets/raw/{}?workspaceId={}&environment={}",
            self.base_url, key, self.project_id, self.environment
        );

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.bearer_token().await))
            .send()
            .await
            .map_err(|e| {
                error!("Infisical fetch_secret failed: {e}");
                CloakError::InfisicalUnavailable
            })?;

        if !resp.status().is_success() {
            return Err(CloakError::Internal(format!("Secret '{key}' not found")));
        }

        #[derive(Deserialize)]
        struct SingleResponse {
            secret: InfisicalSecretRaw,
        }

        let data: SingleResponse = resp.json().await.map_err(|e| {
            CloakError::Internal(format!("Failed to parse secret response: {e}"))
        })?;

        Ok(InfisicalSecret {
            key: data.secret.secret_key,
            value: data.secret.secret_value,
            version: data.secret.version,
        })
    }

    /// Validate a token against Infisical. Token validation is NEVER cached.
    pub async fn validate_token(&self, token: &str) -> Result<TokenValidateResponse, CloakError> {
        let url = format!("{}/api/v1/auth/tokens/validate", self.base_url);

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.bearer_token().await))
            .json(&TokenValidatePayload {
                token: token.to_string(),
            })
            .send()
            .await
            .map_err(|e| {
                error!("Infisical token validation failed: {e}");
                CloakError::InfisicalUnavailable
            })?;

        if !resp.status().is_success() {
            return Ok(TokenValidateResponse {
                valid: false,
                scope: None,
            });
        }

        resp.json().await.map_err(|e| {
            error!("Failed to parse token validation response: {e}");
            CloakError::Internal(format!("Token validation parse error: {e}"))
        })
    }
}
