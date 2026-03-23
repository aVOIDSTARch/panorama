use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use tracing::info;

use cloak_core::{RegistrationRequest, RegistrationResponse};

use crate::state::CloakState;

/// Configuration for connecting to Cloak.
#[derive(Clone)]
pub struct CloakConfig {
    pub cloak_url: String,
    pub manifest_token: String,
    pub service_id: String,
    pub service_type: String,
    pub version: String,
    pub capabilities: Vec<String>,
    /// Maximum consecutive SSE reconnect attempts before self-halt.
    pub sse_max_attempts: u32,
    /// Base delay between SSE reconnect attempts (seconds).
    pub sse_base_delay_secs: u64,
    /// Maximum delay between SSE reconnect attempts (seconds).
    pub sse_max_delay_secs: u64,
}

impl CloakConfig {
    pub fn new(
        cloak_url: impl Into<String>,
        manifest_token: impl Into<String>,
        service_id: impl Into<String>,
        service_type: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            cloak_url: cloak_url.into(),
            manifest_token: manifest_token.into(),
            service_id: service_id.into(),
            service_type: service_type.into(),
            version: version.into(),
            capabilities: Vec::new(),
            sse_max_attempts: 10,
            sse_base_delay_secs: 2,
            sse_max_delay_secs: 60,
        }
    }

    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.capabilities = caps;
        self
    }
}

/// Client for interacting with a Cloak server.
///
/// Handles registration, SSE halt stream listening, and token verification.
pub struct CloakClient {
    config: CloakConfig,
    http: reqwest::Client,
}

impl CloakClient {
    pub fn new(config: CloakConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self { config, http }
    }

    /// Register this service with Cloak. Populates the given state.
    ///
    /// Returns the halt_stream_url for SSE listening.
    /// Fails loudly (Err) if Cloak is unreachable or rejects registration.
    pub async fn register(&self, state: &CloakState) -> Result<String, String> {
        let payload = RegistrationRequest {
            service_id: self.config.service_id.clone(),
            service_type: self.config.service_type.clone(),
            version: self.config.version.clone(),
            capabilities: self.config.capabilities.clone(),
        };

        let url = format!("{}/cloak/services/register", self.config.cloak_url);

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.config.manifest_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Cannot reach Cloak at {}: {e}", self.config.cloak_url))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Cloak registration rejected (HTTP {status}): {body}"
            ));
        }

        let data: RegistrationResponse = resp
            .json()
            .await
            .map_err(|e| format!("Invalid registration response: {e}"))?;

        let signing_key = STANDARD
            .decode(&data.signing_key)
            .map_err(|e| format!("Invalid signing key encoding: {e}"))?;

        state
            .set_registered(data.session_id.clone(), signing_key)
            .await;

        info!(session_id = %data.session_id, "Registered with Cloak");
        Ok(data.halt_stream_url)
    }

    /// Spawn a background task that listens to the SSE halt stream.
    ///
    /// Reconnects with exponential backoff. After max consecutive failures,
    /// self-halts (fail closed) — matching Episteme Python behavior.
    pub fn spawn_halt_listener(&self, state: CloakState, halt_stream_url: String) {
        if halt_stream_url.is_empty() {
            return;
        }

        let config = self.config.clone();

        tokio::spawn(async move {
            crate::sse::listen_halt_stream(config, state, halt_stream_url).await;
        });
    }

    /// Verify a bearer token locally using the signing key from registration.
    ///
    /// Returns the decoded TokenClaims on success.
    pub async fn verify_token(
        &self,
        state: &CloakState,
        token: &str,
    ) -> Result<cloak_core::TokenClaims, cloak_core::CloakError> {
        if state.is_halted().await {
            let reason = state
                .halt_reason()
                .await
                .unwrap_or_else(|| "unknown".into());
            return Err(cloak_core::CloakError::Halted(reason));
        }

        let key = state
            .signing_key()
            .await
            .ok_or(cloak_core::CloakError::NoSigningKey)?;

        let claims = cloak_tokens::signing::verify_and_decode(token, &key)?;

        // Check expiration
        let now = chrono::Utc::now();
        if claims.expires_at < now {
            return Err(cloak_core::CloakError::InvalidToken(
                "Token expired".into(),
            ));
        }

        Ok(claims)
    }

    /// Verify a token and check that it grants access to a specific service.
    ///
    /// Returns the matching ServiceScope.
    pub async fn verify_token_for_service(
        &self,
        state: &CloakState,
        token: &str,
        service_id: &str,
    ) -> Result<(cloak_core::TokenClaims, cloak_core::ServiceScope), cloak_core::CloakError> {
        let claims = self.verify_token(state, token).await?;

        let scope = claims
            .services
            .iter()
            .find(|s| s.service == service_id)
            .cloned()
            .ok_or_else(|| {
                cloak_core::CloakError::ServiceNotInScope(service_id.to_string())
            })?;

        Ok((claims, scope))
    }
}
