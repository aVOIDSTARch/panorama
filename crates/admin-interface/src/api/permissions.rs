use axum::extract::State;
use axum::response::Html;
use axum::Form;
use std::sync::Arc;

use crate::api::health::AppState;

/// HTMX fragment: current permission rules from Cloak.
pub async fn permissions_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    let url = format!("{}/cloak/admin/permissions", state.cloak_url);
    match state.http.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let rules = body.as_array().cloned().unwrap_or_default();

            let mut html = String::new();

            // Add rule form — using concat! to avoid raw string issues with HTML attributes
            html.push_str(concat!(
                "<form hx-post=\"/api/permissions/add\" hx-target=\"#permissions-panel\" hx-swap=\"innerHTML\" class=\"perm-form\">",
                "<input name=\"identity_pattern\" placeholder=\"identity (e.g. * or cortex)\" required />",
                "<input name=\"service\" placeholder=\"service (e.g. episteme)\" required />",
                "<select name=\"operation_class\">",
                "<option value=\"Read\">Read</option>",
                "<option value=\"Write\">Write</option>",
                "<option value=\"Admin\">Admin</option>",
                "</select>",
                "<input name=\"resources\" placeholder=\"resources (* or /api/search)\" />",
                "<button type=\"submit\" class=\"btn btn-add\">Add Rule</button>",
                "</form>",
            ));

            if rules.is_empty() {
                html.push_str("<p class=\"loading\" style=\"margin-top:1rem\">No permission rules configured</p>");
                return Html(html);
            }

            html.push_str("<table style=\"margin-top:1rem\"><thead><tr><th>Identity</th><th>Service</th><th>Operation</th><th>Resources</th><th></th></tr></thead><tbody>");
            for rule in &rules {
                let identity = rule
                    .get("identity_pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let service = rule
                    .get("service")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let op = rule
                    .get("operation_class")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let resources = rule
                    .get("resources")
                    .and_then(|v| {
                        v.as_array().map(|a| {
                            a.iter()
                                .filter_map(|r| r.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                    })
                    .unwrap_or_else(|| "*".to_string());

                html.push_str(&format!(
                    "<tr><td><code>{identity}</code></td><td>{service}</td><td>{op}</td><td><code>{resources}</code></td><td>\
                     <button class=\"btn btn-delete\" \
                     hx-delete=\"/api/permissions/remove?identity_pattern={id_enc}&amp;service={svc_enc}\" \
                     hx-target=\"#permissions-panel\" hx-swap=\"innerHTML\" \
                     hx-confirm=\"Remove rule for {identity} on {service}?\">Remove</button>\
                     </td></tr>",
                    identity = html_escape(identity),
                    service = html_escape(service),
                    op = op,
                    resources = html_escape(&resources),
                    id_enc = urlencod(identity),
                    svc_enc = urlencod(service),
                ));
            }
            html.push_str("</tbody></table>");
            Html(html)
        }
        Err(e) => Html(format!(
            "<p class=\"status-down\">Cannot reach Cloak: {e}</p>"
        )),
        Ok(resp) => {
            let status = resp.status();
            Html(format!(
                "<p class=\"status-down\">Cloak returned HTTP {status}</p>"
            ))
        }
    }
}

#[derive(serde::Deserialize)]
pub struct AddPermission {
    pub identity_pattern: String,
    pub service: String,
    pub operation_class: String,
    pub resources: Option<String>,
}

/// POST /api/permissions/add
pub async fn add_permission(
    State(state): State<Arc<AppState>>,
    Form(form): Form<AddPermission>,
) -> Html<String> {
    let url = format!("{}/cloak/admin/permissions", state.cloak_url);
    let resources: Vec<String> = form
        .resources
        .as_deref()
        .unwrap_or("*")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let body = serde_json::json!({
        "identity_pattern": form.identity_pattern,
        "service": form.service,
        "operation_class": form.operation_class,
        "resources": resources,
    });

    match state.http.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                "Added permission: {} -> {} ({})",
                form.identity_pattern,
                form.service,
                form.operation_class
            );
        }
        Ok(resp) => {
            tracing::error!("Add permission failed: HTTP {}", resp.status());
        }
        Err(e) => {
            tracing::error!("Add permission failed: {e}");
        }
    }

    permissions_panel(State(state)).await
}

#[derive(serde::Deserialize)]
pub struct RemovePermission {
    pub identity_pattern: String,
    pub service: String,
}

/// DELETE /api/permissions/remove
pub async fn remove_permission(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<RemovePermission>,
) -> Html<String> {
    let url = format!("{}/cloak/admin/permissions", state.cloak_url);
    let body = serde_json::json!({
        "identity_pattern": q.identity_pattern,
        "service": q.service,
    });

    match state.http.delete(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                "Removed permission: {} -> {}",
                q.identity_pattern,
                q.service
            );
        }
        Ok(resp) => {
            tracing::error!("Remove permission failed: HTTP {}", resp.status());
        }
        Err(e) => {
            tracing::error!("Remove permission failed: {e}");
        }
    }

    permissions_panel(State(state)).await
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn urlencod(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('*', "%2A")
        .replace('/', "%2F")
}
