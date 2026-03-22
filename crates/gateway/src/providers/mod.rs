pub mod anthropic;
pub mod custom;
pub mod openai;

use crate::types::{Provider, ProviderResponse, Route, SanitizedRequest};
use panorama_errors::PanoramaError;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("timeout")]
    Timeout,
    #[error("rate limited by provider (retry after {retry_after_secs:?}s)")]
    RateLimit { retry_after_secs: Option<u64> },
    #[error("provider server error {status}: {body}")]
    ServerError { status: u16, body: String },
    #[error("connection error: {0}")]
    ConnectionError(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("authentication error: {0}")]
    AuthenticationError(String),
    #[error("missing API key env var: {0}")]
    MissingApiKey(String),
}

/// Dispatch a sanitized request to the appropriate provider based on route config.
pub async fn dispatch(
    client: &reqwest::Client,
    route: &Route,
    request: &SanitizedRequest,
) -> Result<ProviderResponse, ProviderError> {
    // Read API key from environment
    let api_key = std::env::var(&route.api_key_env)
        .map_err(|_| ProviderError::MissingApiKey(route.api_key_env.clone()))?;

    match &route.provider {
        Provider::Anthropic => {
            anthropic::dispatch(client, route, request, &api_key).await
        }
        Provider::OpenAI | Provider::Mistral | Provider::Groq => {
            openai::dispatch(client, route, request, &api_key).await
        }
        Provider::Custom { .. } => {
            custom::dispatch(client, route, request, &api_key).await
        }
    }
}

impl From<ProviderError> for PanoramaError {
    fn from(err: ProviderError) -> Self {
        let (code, detail) = match &err {
            ProviderError::Timeout => ("PROV-001", None),
            ProviderError::RateLimit { retry_after_secs } => {
                ("PROV-002", retry_after_secs.map(|s| format!("retry after {s}s")))
            }
            ProviderError::ServerError { status, body } => {
                ("PROV-003", Some(format!("HTTP {status}: {body}")))
            }
            ProviderError::ConnectionError(d) => ("PROV-004", Some(d.clone())),
            ProviderError::InvalidResponse(d) => ("PROV-005", Some(d.clone())),
            ProviderError::AuthenticationError(d) => ("PROV-006", Some(d.clone())),
            ProviderError::MissingApiKey(d) => ("PROV-007", Some(d.clone())),
        };
        PanoramaError::from_code(code, "gateway", detail)
    }
}
