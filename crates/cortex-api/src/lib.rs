//! Cortex API — central proxy service for Panorama.
//!
//! Routes requests to downstream services (Episteme, Cerebro, Datastore)
//! after validating tokens via Cloak. Manages per-service health state.

pub mod config;
pub mod health;
pub mod proxy;
pub mod router;
pub mod state;

use tracing::info;
use tracing_subscriber::EnvFilter;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Load configuration
    let cfg = config::CortexConfig::from_env()?;
    info!(port = cfg.port, manifest = %cfg.manifest_path, "Cortex starting");

    // Build application state
    let app_state = state::AppState::init(cfg).await?;

    // Spawn health check polling
    health::spawn_health_poller(app_state.clone());

    // Build router
    let app = router::build(app_state.clone());

    // Start server
    let addr = format!("0.0.0.0:{}", app_state.config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(addr = %addr, "Cortex listening");
    axum::serve(listener, app).await?;

    Ok(())
}
