use cortex_core::{ServiceConfig, ServiceManifest};
use std::collections::HashMap;
use tokio::task::JoinHandle;

/// A running Cortex proxy for integration tests.
pub struct TestCortex {
    pub url: String,
    pub port: u16,
    pub handle: JoinHandle<()>,
}

/// Spawn a Cortex proxy on an ephemeral port, registered with the given Cloak.
pub async fn spawn_cortex(cloak_url: &str, manifest: ServiceManifest) -> TestCortex {
    crate::init_tracing();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral port for Cortex");
    let port = listener.local_addr().unwrap().port();

    let config = cortex_api::config::CortexConfig {
        port,
        manifest_path: "test-inline".into(),
        manifest,
        cloak_url: cloak_url.into(),
        cloak_manifest_token: String::new(),
        health_poll_interval_secs: 86400, // effectively disabled during tests
        health_fail_threshold: 100,
    };

    let app_state = cortex_api::state::AppState::init(config)
        .await
        .expect("Failed to init Cortex AppState");

    let app = cortex_api::router::build(app_state);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the background SSE halt listener time to connect to Cloak.
    // Without this, halt events sent immediately after spawn may be missed.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    TestCortex {
        url: format!("http://127.0.0.1:{port}"),
        port,
        handle,
    }
}

/// Build a manifest with a single downstream service pointing at the given URL.
pub fn single_service_manifest(name: &str, base_url: &str) -> ServiceManifest {
    let mut services = HashMap::new();
    services.insert(
        name.to_string(),
        ServiceConfig {
            name: name.to_string(),
            base_url: base_url.to_string(),
            health_path: "/health".into(),
            timeout_ms: 5000,
            queue_ttl_s: 30,
        },
    );
    ServiceManifest { services }
}

/// Build a manifest with episteme, cerebro, and datastore stubs.
pub fn full_stub_manifest(
    episteme_url: &str,
    cerebro_url: &str,
    datastore_url: &str,
) -> ServiceManifest {
    let mut services = HashMap::new();
    for (name, url) in [
        ("episteme", episteme_url),
        ("cerebro", cerebro_url),
        ("datastore", datastore_url),
    ] {
        services.insert(
            name.to_string(),
            ServiceConfig {
                name: name.to_string(),
                base_url: url.to_string(),
                health_path: "/health".into(),
                timeout_ms: 5000,
                queue_ttl_s: 30,
            },
        );
    }
    ServiceManifest { services }
}
