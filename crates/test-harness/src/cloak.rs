use cloak_core::CloakConfig;
use cloak_server::state::AppState;
use tokio::task::JoinHandle;

/// A running Cloak server for integration tests.
pub struct TestCloak {
    pub url: String,
    pub port: u16,
    pub state: AppState,
    pub handle: JoinHandle<()>,
}

/// Spawn a Cloak server on an ephemeral port. No env vars needed.
pub async fn spawn_cloak() -> TestCloak {
    crate::init_tracing();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral port for Cloak");
    let port = listener.local_addr().unwrap().port();

    let config = CloakConfig::for_testing(port);
    let state = AppState::init_with_config(config)
        .await
        .expect("Failed to init Cloak AppState");

    let app = cloak_server::router::build(state.clone());

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    TestCloak {
        url: format!("http://127.0.0.1:{port}"),
        port,
        state,
        handle,
    }
}
