use crate::providers::ProviderError;
use crate::types::{ProviderResponse, Route, SanitizedRequest};
use std::time::Instant;

/// Generic HTTP passthrough for non-standard providers.
/// Sends the prompt as JSON and expects { text, input_tokens, output_tokens } in response.
pub async fn dispatch(
    client: &reqwest::Client,
    route: &Route,
    request: &SanitizedRequest,
    api_key: &str,
) -> Result<ProviderResponse, ProviderError> {
    let start = Instant::now();

    let body = serde_json::json!({
        "prompt": request.prompt,
        "max_tokens": request.options.max_tokens.unwrap_or(route.max_output_tokens),
        "temperature": request.options.temperature,
        "model": route.model_id,
    });

    let resp = client
        .post(&route.endpoint_url)
        .header("authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout
            } else {
                ProviderError::ConnectionError(e.to_string())
            }
        })?;

    let status = resp.status().as_u16();
    let latency_ms = start.elapsed().as_millis() as u64;

    if status == 429 {
        return Err(ProviderError::RateLimit {
            retry_after_secs: None,
        });
    }

    if status >= 400 {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(ProviderError::ServerError {
            status,
            body: body_text,
        });
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

    let response_text = resp_json["text"]
        .as_str()
        .or_else(|| resp_json["response"].as_str())
        .unwrap_or("")
        .to_string();

    let input_tokens = resp_json["input_tokens"].as_u64().unwrap_or(0) as u32;
    let output_tokens = resp_json["output_tokens"].as_u64().unwrap_or(0) as u32;

    Ok(ProviderResponse {
        request_id: request.request_id,
        raw_response: response_text,
        input_tokens,
        output_tokens,
        provider_latency_ms: latency_ms,
        route_key: route.route_key.clone(),
        fallback_attempt: 0,
    })
}
