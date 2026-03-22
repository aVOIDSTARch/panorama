use crate::providers::ProviderError;
use crate::types::{ProviderResponse, Route, SanitizedRequest};
use std::time::Instant;

pub async fn dispatch(
    client: &reqwest::Client,
    route: &Route,
    request: &SanitizedRequest,
    api_key: &str,
) -> Result<ProviderResponse, ProviderError> {
    let start = Instant::now();

    let max_tokens = request
        .options
        .max_tokens
        .unwrap_or(route.max_output_tokens);

    let body = serde_json::json!({
        "model": route.model_id,
        "max_tokens": max_tokens,
        "messages": [
            {
                "role": "user",
                "content": request.prompt,
            }
        ],
        "temperature": request.options.temperature.unwrap_or(1.0),
    });

    let url = format!("{}/v1/messages", route.endpoint_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout
            } else if e.is_connect() {
                ProviderError::ConnectionError(e.to_string())
            } else {
                ProviderError::ConnectionError(e.to_string())
            }
        })?;

    let status = resp.status().as_u16();
    let latency_ms = start.elapsed().as_millis() as u64;

    if status == 429 {
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        return Err(ProviderError::RateLimit {
            retry_after_secs: retry_after,
        });
    }

    if status == 401 || status == 403 {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(ProviderError::AuthenticationError(body_text));
    }

    if status >= 500 {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(ProviderError::ServerError {
            status,
            body: body_text,
        });
    }

    if status >= 400 {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(ProviderError::InvalidResponse(format!(
            "HTTP {status}: {body_text}"
        )));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

    // Extract response text from content array
    let response_text = resp_json["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|block| block["text"].as_str())
        .unwrap_or("")
        .to_string();

    let input_tokens = resp_json["usage"]["input_tokens"]
        .as_u64()
        .unwrap_or(0) as u32;
    let output_tokens = resp_json["usage"]["output_tokens"]
        .as_u64()
        .unwrap_or(0) as u32;

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
