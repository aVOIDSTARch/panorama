pub mod admin;
pub mod health;
pub mod middleware;
pub mod router;
pub mod state;

pub use state::AppState;

pub async fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cloak_server=info,cloak_core=info,tower_http=info".into()),
        )
        .init();

    let state = match AppState::init().await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to initialize Cloak: {e}");
            std::process::exit(1);
        }
    };

    let port = state.config.port;

    // Spawn background secret cache refresh
    let cache = state.secret_cache.clone();
    tokio::spawn(async move {
        cache.background_refresh().await;
    });

    let app = router::build(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind listener");

    tracing::info!("Cloak listening on 0.0.0.0:{port}");
    axum::serve(listener, app).await.expect("Server error");
}
