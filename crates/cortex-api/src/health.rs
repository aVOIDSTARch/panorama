use std::collections::HashMap;

use axum::{extract::State, Json};
use serde_json::{json, Value};
use tracing::{debug, warn};

use crate::state::AppState;

/// Background health poller — checks all downstream services at regular intervals.
pub fn spawn_health_poller(state: AppState) {
    let interval = std::time::Duration::from_secs(state.config.health_poll_interval_secs);

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;

            for (name, svc) in &state.config.manifest.services {
                let url = format!(
                    "{}/{}",
                    svc.base_url.trim_end_matches('/'),
                    svc.health_path.trim_start_matches('/')
                );

                match state.http.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        state.record_health_success(name).await;
                        debug!(service = %name, "Health check OK");
                    }
                    Ok(resp) => {
                        state.record_health_failure(name).await;
                        warn!(service = %name, status = %resp.status(), "Health check failed");
                    }
                    Err(e) => {
                        state.record_health_failure(name).await;
                        warn!(service = %name, error = %e, "Health check unreachable");
                    }
                }
            }
        }
    });
}

/// GET /health — aggregated health status of Cortex and all downstream services.
pub async fn health_handler(State(state): State<AppState>) -> Json<Value> {
    let states = state.service_states.read().await;
    let mut services: HashMap<String, Value> = HashMap::new();

    for (name, svc_state) in states.iter() {
        services.insert(
            name.clone(),
            json!({
                "failure_state": format!("{:?}", svc_state.failure_state),
                "consecutive_failures": svc_state.consecutive_failures,
            }),
        );
    }

    Json(json!({
        "service": "cortex",
        "status": if state.cloak.is_halted().await { "halted" } else { "ok" },
        "registered": state.cloak.is_registered().await,
        "uptime_seconds": state.cloak.uptime_seconds().await,
        "downstream_services": services,
    }))
}
