use crate::types::HealthStatus;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Shared health status map, updated by the health prober background task.
pub type HealthMap = Arc<RwLock<HashMap<String, HealthStatus>>>;

pub fn new_health_map() -> HealthMap {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Check if a route is considered healthy enough for dispatch.
pub fn is_route_healthy(health_map: &HealthMap, route_key: &str) -> bool {
    let map = health_map.read().unwrap();
    match map.get(route_key) {
        Some(HealthStatus::Unhealthy) => false,
        _ => true, // Healthy, Degraded, or unknown (not yet probed) are all dispatchable
    }
}

/// Start background health probe tasks for all active routes.
/// Each route gets its own tokio task that periodically probes the provider.
pub fn start_health_probes(
    health_map: HealthMap,
    routes: Vec<crate::types::Route>,
    client: reqwest::Client,
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut handles = Vec::new();

    for route in routes {
        if !route.active {
            continue;
        }
        let health_map = health_map.clone();
        let client = client.clone();
        let interval_secs = route.health_probe_interval_secs;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;

                let api_key = match std::env::var(&route.api_key_env) {
                    Ok(k) => k,
                    Err(_) => {
                        tracing::warn!(route = %route.route_key, "health probe skipped: missing API key env");
                        let mut map = health_map.write().unwrap();
                        map.insert(route.route_key.clone(), HealthStatus::Unhealthy);
                        continue;
                    }
                };

                // Construct minimal probe request
                let probe_request = crate::types::SanitizedRequest {
                    request_id: uuid::Uuid::new_v4(),
                    route_key: route.route_key.clone(),
                    prompt: "Hi".to_string(),
                    caller_id: "_health_probe".to_string(),
                    session_id: None,
                    options: crate::types::RequestOptions::default(),
                    inbound_hash: String::new(),
                    received_at: chrono::Utc::now(),
                };

                let result = match &route.provider {
                    crate::types::Provider::Anthropic => {
                        crate::providers::anthropic::dispatch(&client, &route, &probe_request, &api_key).await
                    }
                    _ => {
                        crate::providers::openai::dispatch(&client, &route, &probe_request, &api_key).await
                    }
                };

                let new_status = match result {
                    Ok(_) => HealthStatus::Healthy,
                    Err(crate::providers::ProviderError::RateLimit { .. }) => HealthStatus::Degraded,
                    Err(_) => HealthStatus::Unhealthy,
                };

                let mut map = health_map.write().unwrap();
                let old = map.insert(route.route_key.clone(), new_status);
                if old != Some(new_status) {
                    tracing::info!(
                        route = %route.route_key,
                        status = ?new_status,
                        "health status changed"
                    );
                }
            }
        });

        handles.push(handle);
    }

    handles
}
