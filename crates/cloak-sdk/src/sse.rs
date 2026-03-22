use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use tracing::{error, info, warn};

use cloak_core::HaltEvent;

use crate::client::CloakConfig;
use crate::state::CloakState;

/// Persistent SSE listener for halt and key_rotation signals.
///
/// Reconnects with exponential backoff. After max consecutive failures,
/// self-halts (fail closed) — matching Episteme Python client behavior.
pub(crate) async fn listen_halt_stream(
    http: reqwest::Client,
    config: CloakConfig,
    state: CloakState,
    halt_stream_url: String,
) {
    let mut consecutive_failures: u32 = 0;
    let mut delay_secs = config.sse_base_delay_secs;

    loop {
        match connect_and_stream(&http, &config, &state, &halt_stream_url).await {
            Ok(()) => {
                // Stream ended cleanly (server closed). Reset backoff and reconnect.
                consecutive_failures = 0;
                delay_secs = config.sse_base_delay_secs;
                info!("SSE stream ended cleanly, reconnecting");
            }
            Err(e) => {
                consecutive_failures += 1;
                warn!(
                    attempt = consecutive_failures,
                    max = config.sse_max_attempts,
                    error = %e,
                    "SSE connection lost"
                );

                if consecutive_failures >= config.sse_max_attempts {
                    state.set_halted("sse_channel_lost".into()).await;
                    error!("SSE reconnect limit reached — self-halting (fail closed)");
                    return;
                }

                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                delay_secs = (delay_secs * 2).min(config.sse_max_delay_secs);
            }
        }
    }
}

async fn connect_and_stream(
    http: &reqwest::Client,
    config: &CloakConfig,
    state: &CloakState,
    url: &str,
) -> Result<(), String> {
    let resp = http
        .get(url)
        .bearer_auth(&config.manifest_token)
        .send()
        .await
        .map_err(|e| format!("SSE connect failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("SSE HTTP {}", resp.status()));
    }

    info!("SSE halt channel connected");

    // Read the streaming response line by line
    let mut buffer = String::new();
    let mut stream = resp.bytes_stream();

    use tokio_stream::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("SSE read error: {e}"))?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        // Process complete lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue; // SSE comment or blank line (keepalive)
            }

            if let Some(data) = line.strip_prefix("data: ") {
                handle_sse_event(state, data).await;
            }
        }
    }

    Ok(())
}

async fn handle_sse_event(state: &CloakState, raw: &str) {
    let event: HaltEvent = match serde_json::from_str(raw) {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "Failed to parse SSE event");
            return;
        }
    };

    match event.event_type.as_str() {
        "halt" => {
            let reason = event.reason.unwrap_or_else(|| "operator".into());
            state.set_halted(reason.clone()).await;
            warn!(reason = %reason, "HALT received");
        }
        "key_rotation" => {
            if let Some(new_key_b64) = event.new_key {
                match STANDARD.decode(&new_key_b64) {
                    Ok(new_key) => {
                        state.rotate_key(new_key).await;
                        info!("Signing key rotated");
                    }
                    Err(e) => {
                        warn!(error = %e, "key_rotation event has invalid base64 key");
                    }
                }
            } else {
                warn!("key_rotation event missing new_key");
            }
        }
        other => {
            warn!(event_type = %other, "Unknown SSE event type");
        }
    }
}
