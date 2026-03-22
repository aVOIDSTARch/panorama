use axum::{extract::State, response::Html};
use std::sync::Arc;

/// Check health of all downstream services and return an HTMX fragment.
pub async fn health_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    let services = [
        ("Cloak", &state.cloak_url),
        ("Cortex", &state.cortex_url),
        ("Episteme", &state.episteme_url),
        ("Cerebro", &state.cerebro_url),
        ("Datastore", &state.datastore_url),
        ("Gateway", &state.gateway_url),
    ];

    let mut rows = String::new();
    for (name, url) in &services {
        let health_url = format!("{url}/health");
        let (status_class, status_text) = match state.http.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => ("status-ok", "healthy"),
            Ok(_) => ("status-degraded", "degraded"),
            Err(_) => ("status-down", "unreachable"),
        };
        rows.push_str(&format!(
            r#"<tr><td>{name}</td><td class="{status_class}">{status_text}</td><td><code>{url}</code></td></tr>"#,
        ));
    }

    Html(format!(
        r#"<table><thead><tr><th>Service</th><th>Status</th><th>URL</th></tr></thead><tbody>{rows}</tbody></table>"#,
    ))
}

/// Return list of Cloak-registered services as an HTMX fragment.
pub async fn services_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    let url = format!("{}/cloak/services", state.cloak_url);
    match state.http.get(&url).send().await {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let services = body.as_array().cloned().unwrap_or_default();
            let mut rows = String::new();
            for svc in &services {
                let id = svc.get("service_id").and_then(|v| v.as_str()).unwrap_or("?");
                let stype = svc.get("service_type").and_then(|v| v.as_str()).unwrap_or("?");
                let version = svc.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                rows.push_str(&format!(
                    "<tr><td>{id}</td><td>{stype}</td><td>{version}</td></tr>"
                ));
            }
            Html(format!(
                r#"<table><thead><tr><th>Service</th><th>Type</th><th>Version</th></tr></thead><tbody>{rows}</tbody></table>"#,
            ))
        }
        Err(e) => Html(format!("<p class='status-down'>Cannot reach Cloak: {e}</p>")),
    }
}

pub struct AppState {
    pub http: reqwest::Client,
    pub cloak_url: String,
    pub cortex_url: String,
    pub episteme_url: String,
    pub cerebro_url: String,
    pub datastore_url: String,
    pub gateway_url: String,
}

impl AppState {
    pub fn from_env() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("failed to build HTTP client"),
            cloak_url: std::env::var("CLOAK_URL").unwrap_or_else(|_| "http://localhost:8300".into()),
            cortex_url: std::env::var("CORTEX_URL").unwrap_or_else(|_| "http://localhost:9000".into()),
            episteme_url: std::env::var("EPISTEME_URL").unwrap_or_else(|_| "http://localhost:8100".into()),
            cerebro_url: std::env::var("CEREBRO_URL").unwrap_or_else(|_| "http://localhost:8101".into()),
            datastore_url: std::env::var("DATASTORE_URL").unwrap_or_else(|_| "http://localhost:8102".into()),
            gateway_url: std::env::var("GATEWAY_URL").unwrap_or_else(|_| "http://localhost:8800".into()),
        }
    }
}
