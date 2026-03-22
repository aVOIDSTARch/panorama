use axum::extract::Request;
use axum::routing::get;
use axum::{Json, Router};
use tokio::task::JoinHandle;

/// A stub downstream service that returns 200 for /health and echoes
/// request info for all other paths.
pub struct StubService {
    pub url: String,
    pub port: u16,
    pub handle: JoinHandle<()>,
}

pub async fn spawn_stub(name: &str) -> StubService {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral port for stub");
    let port = listener.local_addr().unwrap().port();

    let service_name = name.to_string();
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .fallback(move |req: Request| {
            let name = service_name.clone();
            async move {
                Json(serde_json::json!({
                    "stub": true,
                    "service": name,
                    "path": req.uri().path().to_string(),
                    "method": req.method().to_string(),
                }))
            }
        });

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    StubService {
        url: format!("http://127.0.0.1:{port}"),
        port,
        handle,
    }
}
