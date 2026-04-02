use axum::{extract::State, http::StatusCode, response::IntoResponse, routing, Json, Router};
use serde_json::json;
use tower_http::trace::TraceLayer;

use analog_communications::config::AnalogConfig;
use analog_communications::AppState;

#[tokio::main]
async fn main() {
    panorama_logging::init("analog-communications", Some("data/panorama_logs.db"));

    let mut config = AnalogConfig::from_env().expect("failed to load config");
    let port = config.port;

    // Register with Cloak
    let cloak = cloak_sdk::CloakState::new();
    let cloak_config = cloak_sdk::CloakConfig::new(
        &config.cloak_url,
        &config.cloak_manifest_token,
        "analog-communications",
        "inbound",
        env!("CARGO_PKG_VERSION"),
    )
    .with_capabilities(vec!["sms_inbound".into(), "idea_capture".into()]);

    let client = cloak_sdk::CloakClient::new(cloak_config);
    match client.register(&cloak).await {
        Ok(halt_url) => {
            tracing::info!("Registered with Cloak");
            client.spawn_halt_listener(cloak.clone(), halt_url);
        }
        Err(e) => {
            tracing::warn!("Cloak registration failed (continuing without): {e}");
        }
    }

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client");

    // Fetch secrets from Infisical via Cloak (best-effort, overrides env fallbacks)
    config.load_cloak_secrets(&http).await;

    // Load recognized senders from Datastore (best-effort)
    let recognized_senders = load_recognized_senders(&http, config.datastore_url.as_deref()).await;

    let state = AppState {
        config,
        cloak: cloak.clone(),
        http,
        recognized_senders: std::sync::Arc::new(std::sync::Mutex::new(recognized_senders)),
    };

    // Webhook route (Telnyx verifies via its own signature, not Cloak)
    let webhook = Router::new()
        .route("/sms-inbound", routing::post(analog_communications::inbound::sms_inbound));

    // Health (no auth)
    let health = Router::new().route("/health", routing::get(health_handler));

    let app = Router::new()
        .merge(health)
        .merge(webhook)
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Analog communications listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

/// Load recognized senders from Datastore at startup.
/// Returns an empty list if Datastore is unavailable (non-fatal).
async fn load_recognized_senders(http: &reqwest::Client, datastore_url: Option<&str>) -> Vec<String> {
    let url = match datastore_url {
        Some(u) => format!("{u}/query"),
        None => return Vec::new(),
    };

    let query = json!({
        "collection": "_recognized_senders",
        "query": {},
        "limit": 10000,
    });

    match http.post(&url).json(&query).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            body.as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.get("phone").and_then(|p| p.as_str()).map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        }
        Ok(resp) => {
            tracing::warn!(status = %resp.status(), "Failed to load recognized senders from Datastore");
            Vec::new()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Datastore unreachable — starting with empty recognized senders list");
            Vec::new()
        }
    }
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let halted = state.cloak.is_halted().await;
    (
        if halted { StatusCode::SERVICE_UNAVAILABLE } else { StatusCode::OK },
        Json(json!({
            "status": if halted { "halted" } else { "ok" },
            "service": "analog-communications",
            "halted": halted,
        })),
    )
}
