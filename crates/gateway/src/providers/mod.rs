pub mod anthropic;
pub mod custom;
pub mod openai;

use crate::types::{Provider, ProviderResponse, Route, SanitizedRequest};

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
