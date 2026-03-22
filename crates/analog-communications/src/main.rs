use axum::{extract::State, http::StatusCode, response::IntoResponse, routing, Json, Router};
use serde_json::json;
use tower_http::trace::TraceLayer;

use analog_communications::config::AnalogConfig;
use analog_communications::AppState;

#[tokio::main]
async fn main() {
    panorama_logging::init("analog-communications", Some("data/panorama_logs.db"));

    let config = AnalogConfig::from_env().expect("failed to load config");
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

    let state = AppState {
        config,
        cloak: cloak.clone(),
        http,
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
