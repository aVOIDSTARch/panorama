use axum::{middleware, routing, Router};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use admin_interface::api;
use admin_interface::auth;

#[tokio::main]
async fn main() {
    panorama_logging::init("admin-interface", Some("data/panorama_logs.db"));

    let state = Arc::new(api::health::AppState::from_env());
    let port: u16 = std::env::var("ADMIN_PORT")
        .unwrap_or_else(|_| "8400".into())
        .parse()
        .unwrap_or(8400);

    let app = Router::new()
        // Pages
        .route("/", routing::get(api::dashboard))
        .route("/login", routing::get(auth::session::login_page))
        .route("/login", routing::post(auth::session::login_submit))
        // HTMX API fragments
        .route("/api/health", routing::get(api::health::health_panel))
        .route("/api/services", routing::get(api::health::services_panel))
        // Auth middleware (skip for /login, /health, /static)
        .layer(middleware::from_fn(auth::require_auth))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Admin interface listening on {addr} (Tailscale/LAN only)");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}
