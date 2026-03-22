use axum::extract::{Query, State};
use axum::response::Html;
use std::sync::Arc;

use crate::api::health::AppState;

#[derive(serde::Deserialize, Default)]
pub struct LogQuery {
    pub service: Option<String>,
    pub level: Option<String>,
    pub error_code: Option<String>,
    pub limit: Option<u32>,
}

/// HTMX fragment: recent log entries from _system_logs.
pub async fn logs_panel(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LogQuery>,
) -> Html<String> {
    let db = match &state.log_db {
        Some(db) => db,
        None => return Html("<p class='status-down'>Log database not available</p>".into()),
    };

    let conn = db.lock().unwrap();
    let limit = q.limit.unwrap_or(50).min(200);

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref svc) = q.service {
        if !svc.is_empty() {
            conditions.push(format!("service = ?{}", params.len() + 1));
            params.push(Box::new(svc.clone()));
        }
    }
    if let Some(ref lvl) = q.level {
        if !lvl.is_empty() {
            conditions.push(format!("level = ?{}", params.len() + 1));
            params.push(Box::new(lvl.clone()));
        }
    }
    if let Some(ref code) = q.error_code {
        if !code.is_empty() {
            conditions.push(format!("error_code LIKE ?{}", params.len() + 1));
            params.push(Box::new(format!("{code}%")));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT timestamp, level, service, target, message, error_code
         FROM _system_logs {where_clause}
         ORDER BY timestamp DESC LIMIT {limit}"
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let result = conn.prepare(&sql).and_then(|mut stmt| {
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(LogRow {
                timestamp: row.get(0)?,
                level: row.get(1)?,
                service: row.get(2)?,
                target: row.get(3)?,
                message: row.get(4)?,
                error_code: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    });

    match result {
        Ok(rows) => {
            if rows.is_empty() {
                return Html("<p class='loading'>No log entries found</p>".into());
            }
            let mut html = String::from(
                "<table><thead><tr><th>Time</th><th>Level</th><th>Service</th><th>Code</th><th>Message</th></tr></thead><tbody>",
            );
            for row in &rows {
                let level_class = match row.level.as_str() {
                    "ERROR" => "status-down",
                    "WARN" => "status-degraded",
                    _ => "",
                };
                let code_display = row.error_code.as_deref().unwrap_or("");
                let ts = &row.timestamp[..19.min(row.timestamp.len())]; // trim to seconds
                let msg = truncate(&row.message, 120);
                html.push_str(&format!(
                    r#"<tr><td><code>{ts}</code></td><td class="{level_class}">{}</td><td>{}</td><td><code>{code_display}</code></td><td>{msg}</td></tr>"#,
                    row.level, row.service,
                ));
            }
            html.push_str("</tbody></table>");
            Html(html)
        }
        Err(e) => Html(format!("<p class='status-down'>Query error: {e}</p>")),
    }
}

struct LogRow {
    timestamp: String,
    level: String,
    service: String,
    #[allow(dead_code)]
    target: String,
    message: String,
    error_code: Option<String>,
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        html_escape(s)
    } else {
        format!("{}...", html_escape(&s[..max]))
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
