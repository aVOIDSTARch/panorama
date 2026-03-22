use test_harness::cloak::spawn_cloak;
use test_harness::cortex::{full_stub_manifest, spawn_cortex};
use test_harness::stub_service::spawn_stub;

#[tokio::test]
async fn cloak_health_returns_200() {
    let cloak = spawn_cloak().await;
    let resp = reqwest::get(format!("{}/health", cloak.url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["service_id"], "cloak");
    assert_eq!(body["halted"], false);
    // Infisical is unreachable in test — that's expected
    assert_eq!(body["infisical_reachable"], false);
}

#[tokio::test]
async fn service_registration_returns_session_and_key() {
    let cloak = spawn_cloak().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/cloak/services/register", cloak.url))
        .json(&cloak_core::RegistrationRequest {
            service_id: "test-svc".into(),
            service_type: "worker".into(),
            version: "0.1.0".into(),
            capabilities: vec!["compute".into()],
        })
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: cloak_core::RegistrationResponse = resp.json().await.unwrap();
    assert!(!body.session_id.is_empty());
    assert!(!body.signing_key.is_empty());
    assert!(body.halt_stream_url.contains("halt-stream"));

    // Verify it appears in service list
    let list: Vec<String> = client
        .get(format!("{}/cloak/services", cloak.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.contains(&"test-svc".to_string()));
}

#[tokio::test]
async fn cortex_registers_with_cloak_on_startup() {
    let cloak = spawn_cloak().await;

    let stub_e = spawn_stub("episteme").await;
    let stub_c = spawn_stub("cerebro").await;
    let stub_d = spawn_stub("datastore").await;

    let manifest = full_stub_manifest(&stub_e.url, &stub_c.url, &stub_d.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let client = reqwest::Client::new();

    // Cortex should have registered with Cloak during init
    let services: Vec<String> = client
        .get(format!("{}/cloak/services", cloak.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(services.contains(&"cortex".to_string()));

    // Cortex health should show registered
    let health: serde_json::Value = client
        .get(format!("{}/health", cortex.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["status"], "ok");
    assert_eq!(health["registered"], true);
}

#[tokio::test]
async fn cortex_health_shows_downstream_services() {
    let cloak = spawn_cloak().await;

    let stub_e = spawn_stub("episteme").await;
    let stub_c = spawn_stub("cerebro").await;

    let manifest = test_harness::cortex::full_stub_manifest(&stub_e.url, &stub_c.url, &stub_e.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let health: serde_json::Value = reqwest::get(format!("{}/health", cortex.url))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let downstream = &health["downstream_services"];
    assert!(downstream["episteme"].is_object());
    assert!(downstream["cerebro"].is_object());
    assert_eq!(downstream["episteme"]["failure_state"], "Healthy");
}

#[tokio::test]
async fn cloak_health_counts_registered_services() {
    let cloak = spawn_cloak().await;
    let client = reqwest::Client::new();

    // Register two services
    for id in ["svc-a", "svc-b"] {
        client
            .post(format!("{}/cloak/services/register", cloak.url))
            .json(&cloak_core::RegistrationRequest {
                service_id: id.into(),
                service_type: "worker".into(),
                version: "0.1.0".into(),
                capabilities: vec![],
            })
            .send()
            .await
            .unwrap();
    }

    let health: serde_json::Value = client
        .get(format!("{}/health", cloak.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["registered_services"], 2);
}
