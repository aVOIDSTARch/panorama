use axum::extract::State;
use axum::response::Html;
use std::sync::Arc;

use crate::api::health::AppState;

/// HTMX fragment: identity panel showing allowed senders and recognized senders.
pub async fn identity_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    let mut html = String::new();

    // Section 1: Allowed senders from config (ANALOG_ALLOWED_SENDERS)
    let allowed_raw =
        std::env::var("ANALOG_ALLOWED_SENDERS").unwrap_or_default();
    let allowed: Vec<&str> = allowed_raw
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let owner = std::env::var("ANALOG_OWNER_NUMBER").unwrap_or_default();

    html.push_str("<h4>Owner</h4>");
    if owner.is_empty() {
        html.push_str("<p class='loading'>No owner number configured (ANALOG_OWNER_NUMBER)</p>");
    } else {
        html.push_str(&format!(
            "<p><code>{}</code> <span class='status-ok'>(TOTP-protected)</span></p>",
            html_escape(&owner)
        ));
    }

    html.push_str("<h4 style='margin-top:1rem'>Allowed Senders</h4>");
    if allowed.is_empty() {
        html.push_str("<p class='loading'>No allowed senders configured (ANALOG_ALLOWED_SENDERS)</p>");
    } else {
        html.push_str("<table><thead><tr><th>Phone</th><th>Status</th></tr></thead><tbody>");
        for phone in &allowed {
            html.push_str(&format!(
                "<tr><td><code>{}</code></td><td class='status-ok'>allowed</td></tr>",
                html_escape(phone)
            ));
        }
        html.push_str("</tbody></table>");
    }

    // Section 2: Recognized senders from Datastore
    html.push_str("<h4 style='margin-top:1rem'>Recognized Senders</h4>");

    let datastore_url = std::env::var("DATASTORE_URL")
        .unwrap_or_else(|_| "http://localhost:8102".into());
    let query_url = format!("{datastore_url}/query");

    let query = serde_json::json!({
        "collection": "_recognized_senders",
        "query": {},
        "limit": 100,
    });

    match state.http.post(&query_url).json(&query).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let senders = body.as_array().cloned().unwrap_or_default();

            if senders.is_empty() {
                html.push_str("<p class='loading'>No recognized senders yet</p>");
            } else {
                html.push_str(
                    "<table><thead><tr>\
                     <th>Phone</th><th>Last Seen</th>\
                     </tr></thead><tbody>",
                );
                for sender in &senders {
                    let phone = sender
                        .get("phone")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let last_seen = sender
                        .get("last_seen")
                        .and_then(|v| v.as_str())
                        .map(|s| if s.len() > 19 { &s[..19] } else { s })
                        .unwrap_or("?");
                    html.push_str(&format!(
                        "<tr><td><code>{}</code></td><td><code>{}</code></td></tr>",
                        html_escape(phone),
                        html_escape(last_seen),
                    ));
                }
                html.push_str("</tbody></table>");
            }
        }
        Ok(_) => {
            html.push_str("<p class='status-degraded'>Datastore returned an error</p>");
        }
        Err(_) => {
            html.push_str("<p class='status-down'>Datastore unreachable</p>");
        }
    }

    Html(html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
