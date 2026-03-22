use test_harness::cloak::spawn_cloak;
use test_harness::cortex::{single_service_manifest, spawn_cortex};
use test_harness::stub_service::spawn_stub;
use test_harness::tokens::{sign_expired_token, sign_test_token};

#[tokio::test]
async fn proxy_with_valid_token_forwards_to_downstream() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let _cortex = spawn_cortex(&cloak.url, manifest).await;

    // Get Cortex's per-session signing key from Cloak's registry
    let cortex_entry = cloak.state.registry.get("cortex").unwrap();
    let token = sign_test_token(&cortex_entry.signing_key, "episteme");

    let resp = reqwest::Client::new()
        .get(format!("{}/episteme/some/path", _cortex.url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["stub"], true);
    assert_eq!(body["service"], "episteme");
    assert_eq!(body["path"], "/some/path");
}

#[tokio::test]
async fn proxy_rejects_missing_token() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let resp = reqwest::Client::new()
        .get(format!("{}/episteme/data", cortex.url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "missing_token");
}

#[tokio::test]
async fn proxy_rejects_expired_token() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let cortex_entry = cloak.state.registry.get("cortex").unwrap();
    let token = sign_expired_token(&cortex_entry.signing_key, "episteme");

    let resp = reqwest::Client::new()
        .get(format!("{}/episteme/data", cortex.url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "invalid_token");
}

#[tokio::test]
async fn proxy_rejects_wrong_signing_key() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    // Sign with a random key, not Cortex's per-session key
    let wrong_key = cloak_tokens::signing::generate_signing_key();
    let token = sign_test_token(&wrong_key, "episteme");

    let resp = reqwest::Client::new()
        .get(format!("{}/episteme/data", cortex.url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "invalid_signature");
}

#[tokio::test]
async fn proxy_unknown_service_returns_404() {
    let cloak = spawn_cloak().await;
    // Empty manifest — no services configured
    let manifest = cortex_core::ServiceManifest {
        services: std::collections::HashMap::new(),
    };
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let cortex_entry = cloak.state.registry.get("cortex").unwrap();
    let token = sign_test_token(&cortex_entry.signing_key, "nonexistent");

    let resp = reqwest::Client::new()
        .get(format!("{}/nonexistent/data", cortex.url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "service_not_found");
}

#[tokio::test]
async fn health_endpoint_works_without_token() {
    let cloak = spawn_cloak().await;
    let manifest = cortex_core::ServiceManifest {
        services: std::collections::HashMap::new(),
    };
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    // No auth header — health should still work
    let resp = reqwest::get(format!("{}/health", cortex.url))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}
