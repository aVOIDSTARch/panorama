use axum::extract::State;
use axum::response::Html;
use std::sync::Arc;

use crate::api::health::AppState;

/// HTMX fragment: Wheelhouse pool status and agent list.
pub async fn wheelhouse_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    // Fetch status
    let status_url = format!("{}/status", state.wheelhouse_url());
    let status = match state.http.get(&status_url).send().await {
        Ok(resp) if resp.status().is_success() => resp.json::<serde_json::Value>().await.ok(),
        _ => None,
    };

    // Fetch agents list
    let agents_url = format!("{}/agents", state.wheelhouse_url());
    let agents: Vec<serde_json::Value> = match state.http.get(&agents_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            body.get("agents")
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };

    let mut html = String::new();

    // Pool summary
    if let Some(ref s) = status {
        let pool = s.get("pool").cloned().unwrap_or_default();
        let total = pool.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        let idle = pool.get("idle").and_then(|v| v.as_u64()).unwrap_or(0);
        let active = pool.get("active").and_then(|v| v.as_u64()).unwrap_or(0);
        let retiring = pool.get("retiring").and_then(|v| v.as_u64()).unwrap_or(0);

        html.push_str(&format!(
            "<div class=\"pool-summary\">\
             <span class=\"stat\">Total: <strong>{total}</strong></span>\
             <span class=\"stat status-ok\">Idle: <strong>{idle}</strong></span>\
             <span class=\"stat status-degraded\">Active: <strong>{active}</strong></span>\
             <span class=\"stat\">Retiring: <strong>{retiring}</strong></span>\
             </div>"
        ));
    } else {
        html.push_str("<p class='status-down'>Wheelhouse unreachable</p>");
        return Html(html);
    }

    // Agent table
    if agents.is_empty() {
        html.push_str("<p class='loading'>No agents in pool</p>");
    } else {
        html.push_str(
            "<table><thead><tr>\
             <th>ID</th><th>Tier</th><th>Model</th><th>Status</th>\
             <th>Task</th><th>Completed</th><th>Tokens</th><th>Spawned</th>\
             </tr></thead><tbody>"
        );

        for agent in &agents {
            let id = agent.get("agent_id").and_then(|v| v.as_str()).unwrap_or("?");
            let short_id = if id.len() > 8 { &id[..8] } else { id };
            let tier = agent.get("tier").and_then(|v| v.as_str()).unwrap_or("?");
            let model = agent.get("model_id").and_then(|v| v.as_str()).unwrap_or("?");
            let status = agent.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            let task = agent
                .get("current_task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("-");
            let completed = agent
                .get("tasks_completed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let tokens = agent
                .get("total_tokens_used")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let spawned = agent
                .get("spawned_at")
                .and_then(|v| v.as_str())
                .map(|s| if s.len() > 19 { &s[..19] } else { s })
                .unwrap_or("?");

            let status_class = match status {
                "idle" => "status-ok",
                "active" => "status-degraded",
                "retiring" | "dead" => "status-down",
                _ => "",
            };

            let short_task = if task.len() > 8 { &task[..8] } else { task };

            html.push_str(&format!(
                "<tr>\
                 <td><code>{short_id}</code></td>\
                 <td>{tier}</td>\
                 <td><code>{model}</code></td>\
                 <td class=\"{status_class}\">{status}</td>\
                 <td><code>{short_task}</code></td>\
                 <td>{completed}</td>\
                 <td>{tokens}</td>\
                 <td><code>{spawned}</code></td>\
                 </tr>"
            ));
        }

        html.push_str("</tbody></table>");
    }

    Html(html)
}

impl AppState {
    /// Get wheelhouse URL (defaults to Cortex proxy path).
    fn wheelhouse_url(&self) -> String {
        // Admin interface talks to Wheelhouse directly or via Cortex
        std::env::var("WHEELHOUSE_URL").unwrap_or_else(|_| "http://localhost:8200".into())
    }
}
