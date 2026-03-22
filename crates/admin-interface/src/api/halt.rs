use axum::extract::{Path, State};
use axum::response::Html;
use std::sync::Arc;

use crate::api::health::AppState;

/// HTMX fragment: halt status + control buttons.
pub async fn halt_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    let health_url = format!("{}/health", state.cloak_url);
    let (halted, reason) = match state.http.get(&health_url).send().await {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let halted = body.get("halted").and_then(|v| v.as_bool()).unwrap_or(false);
            let reason = body
                .get("halt_reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (halted, reason)
        }
        Err(_) => (false, Some("Cloak unreachable".to_string())),
    };

    let services_url = format!("{}/cloak/services", state.cloak_url);
    let service_ids: Vec<String> = match state.http.get(&services_url).send().await {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            body.as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|s| {
                    s.get("service_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        }
        Err(_) => vec![],
    };

    let mut html = String::new();

    if halted {
        let reason_text = reason.as_deref().unwrap_or("unknown");
        html.push_str(&format!(
            "<div class=\"halt-status halt-active\">HALTED — {}</div>",
            html_escape(reason_text),
        ));
        html.push_str(concat!(
            "<button class=\"btn btn-resume\" ",
            "hx-post=\"/api/halt/resume\" hx-target=\"#halt-panel\" hx-swap=\"innerHTML\" ",
            "hx-confirm=\"Resume all services from halt?\">Resume</button>",
        ));
    } else {
        html.push_str("<div class=\"halt-status halt-ok\">System Normal</div>");
    }

    html.push_str(concat!(
        "<div style=\"margin-top:1rem\">",
        "<button class=\"btn btn-halt\" ",
        "hx-post=\"/api/halt/all\" hx-target=\"#halt-panel\" hx-swap=\"innerHTML\" ",
        "hx-confirm=\"HALT ALL SERVICES? This stops all request processing.\">",
        "Halt All Services</button></div>",
    ));

    if !service_ids.is_empty() {
        html.push_str("<div style=\"margin-top:1rem\"><strong>Per-service halt:</strong><div style=\"margin-top:0.5rem\">");
        for sid in &service_ids {
            html.push_str(&format!(
                concat!(
                    "<button class=\"btn btn-halt-sm\" ",
                    "hx-post=\"/api/halt/service/{sid}\" hx-target=\"#halt-panel\" hx-swap=\"innerHTML\" ",
                    "hx-confirm=\"Halt service {sid}?\">{sid}</button> "
                ),
                sid = html_escape(sid),
            ));
        }
        html.push_str("</div></div>");
    }

    Html(html)
}

/// POST /api/halt/all
pub async fn halt_all(State(state): State<Arc<AppState>>) -> Html<String> {
    let url = format!("{}/cloak/admin/halt", state.cloak_url);
    let body = serde_json::json!({"reason": "admin-interface operator"});
    match state.http.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::warn!("Operator issued HALT ALL via admin interface");
        }
        Ok(resp) => {
            tracing::error!("Halt all failed: HTTP {}", resp.status());
        }
        Err(e) => {
            tracing::error!("Halt all failed: {e}");
        }
    }
    halt_panel(State(state)).await
}

/// POST /api/halt/resume
pub async fn resume(State(state): State<Arc<AppState>>) -> Html<String> {
    let url = format!("{}/cloak/admin/resume", state.cloak_url);
    match state.http.post(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Operator issued RESUME via admin interface");
        }
        Ok(resp) => {
            tracing::error!("Resume failed: HTTP {}", resp.status());
        }
        Err(e) => {
            tracing::error!("Resume failed: {e}");
        }
    }
    halt_panel(State(state)).await
}

/// POST /api/halt/service/:service_id
pub async fn halt_service(
    State(state): State<Arc<AppState>>,
    Path(service_id): Path<String>,
) -> Html<String> {
    let url = format!("{}/cloak/admin/halt/{service_id}", state.cloak_url);
    let body = serde_json::json!({"reason": "admin-interface operator"});
    match state.http.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::warn!("Operator halted service {service_id} via admin interface");
        }
        Ok(resp) => {
            tracing::error!("Halt {service_id} failed: HTTP {}", resp.status());
        }
        Err(e) => {
            tracing::error!("Halt {service_id} failed: {e}");
        }
    }
    halt_panel(State(state)).await
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
