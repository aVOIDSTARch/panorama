use test_harness::cloak::spawn_cloak;
use test_harness::cortex::{single_service_manifest, spawn_cortex};
use test_harness::stub_service::spawn_stub;
use test_harness::tokens::sign_test_token;

#[tokio::test]
async fn global_halt_stops_proxy_requests() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let client = reqwest::Client::new();

    // Verify proxy works before halt
    let cortex_entry = cloak.state.registry.get("cortex").unwrap();
    let token = sign_test_token(&cortex_entry.signing_key, "episteme");

    let resp = client
        .get(format!("{}/episteme/data", cortex.url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Issue global halt
    let halt_resp = client
        .post(format!("{}/cloak/admin/halt", cloak.url))
        .json(&serde_json::json!({ "reason": "emergency-test" }))
        .send()
        .await
        .unwrap();
    assert_eq!(halt_resp.status(), 200);

    // Wait for SSE propagation
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Proxy requests should now be rejected with 503
    let resp = client
        .get(format!("{}/episteme/data", cortex.url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "service_halted");
}

#[tokio::test]
async fn health_endpoint_survives_halt() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let client = reqwest::Client::new();

    // Issue halt
    client
        .post(format!("{}/cloak/admin/halt", cloak.url))
        .json(&serde_json::json!({ "reason": "test" }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Health should still respond, but show halted status
    let health: serde_json::Value = client
        .get(format!("{}/health", cortex.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(health["status"], "halted");
}

#[tokio::test]
async fn per_service_halt_targets_specific_service() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let client = reqwest::Client::new();

    // Halt only the "cortex" service
    let resp = client
        .post(format!("{}/cloak/admin/halt/cortex", cloak.url))
        .json(&serde_json::json!({ "reason": "targeted-halt" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Cortex should be halted
    let health: serde_json::Value = client
        .get(format!("{}/health", cortex.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["status"], "halted");
}

#[tokio::test]
async fn halt_is_permanent_no_sse_resume() {
    let cloak = spawn_cloak().await;
    let stub = spawn_stub("episteme").await;
    let manifest = single_service_manifest("episteme", &stub.url);
    let cortex = spawn_cortex(&cloak.url, manifest).await;

    let client = reqwest::Client::new();

    // Halt
    client
        .post(format!("{}/cloak/admin/halt", cloak.url))
        .json(&serde_json::json!({ "reason": "test" }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Resume at Cloak level (clears Cloak's own flag)
    client
        .post(format!("{}/cloak/admin/resume", cloak.url))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Cortex stays halted — there is no SSE resume event.
    // This is by design: halt is fail-closed, services must restart.
    let health: serde_json::Value = client
        .get(format!("{}/health", cortex.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["status"], "halted");
}

#[tokio::test]
async fn cloak_halt_nonexistent_service_returns_404() {
    let cloak = spawn_cloak().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/cloak/admin/halt/ghost", cloak.url))
        .json(&serde_json::json!({ "reason": "test" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}
