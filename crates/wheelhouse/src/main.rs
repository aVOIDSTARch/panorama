use axum::{middleware, routing, Router};
use tower_http::trace::TraceLayer;

use wheelhouse::config::WheelhouseConfig;
use wheelhouse::hub;
use wheelhouse::state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = WheelhouseConfig::from_env().expect("failed to load config");
    let port = config.port;

    let state = AppState::init(config)
        .await
        .expect("failed to initialize Wheelhouse");

    // Authenticated routes
    let api = Router::new()
        .route("/jobs", routing::post(hub::submit_job))
        .route("/status", routing::get(hub::status))
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::cloak_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::halt_guard,
        ));

    // Health (no auth)
    let health = Router::new().route("/health", routing::get(hub::health));

    let app = Router::new()
        .merge(health)
        .merge(api)
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Wheelhouse listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}
