use axum::extract::{Query, State};
use axum::response::Html;
use std::sync::Arc;

use crate::api::health::AppState;

#[derive(serde::Deserialize, Default)]
pub struct ErrorQuery {
    pub service: Option<String>,
    pub severity: Option<String>,
    pub code: Option<String>,
    pub limit: Option<u32>,
}

/// HTMX fragment: error report summary grouped by code.
pub async fn errors_summary_panel(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ErrorQuery>,
) -> Html<String> {
    let db = match &state.log_db {
        Some(db) => db,
        None => return Html("<p class='status-down'>Log database not available</p>".into()),
    };

    let conn = db.lock().unwrap();

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref svc) = q.service {
        if !svc.is_empty() {
            conditions.push(format!("service = ?{}", params.len() + 1));
            params.push(Box::new(svc.clone()));
        }
    }
    if let Some(ref sev) = q.severity {
        if !sev.is_empty() {
            conditions.push(format!("severity = ?{}", params.len() + 1));
            params.push(Box::new(sev.clone()));
        }
    }
    if let Some(ref code) = q.code {
        if !code.is_empty() {
            conditions.push(format!("code LIKE ?{}", params.len() + 1));
            params.push(Box::new(format!("{code}%")));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT code, message, severity, service, suggestion,
                COUNT(*) as count,
                MAX(timestamp) as last_seen,
                retryable
         FROM _error_reports {where_clause}
         GROUP BY code
         ORDER BY count DESC
         LIMIT 50"
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let result = conn.prepare(&sql).and_then(|mut stmt| {
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(ErrorSummary {
                code: row.get(0)?,
                message: row.get(1)?,
                severity: row.get(2)?,
                service: row.get(3)?,
                suggestion: row.get(4)?,
                count: row.get(5)?,
                last_seen: row.get(6)?,
                retryable: row.get::<_, i32>(7)? != 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    });

    match result {
        Ok(rows) => {
            if rows.is_empty() {
                return Html("<p class='loading'>No error reports recorded</p>".into());
            }
            let mut html = String::from(
                "<table><thead><tr><th>Code</th><th>Message</th><th>Severity</th><th>Service</th><th>Count</th><th>Last Seen</th><th>Retryable</th></tr></thead><tbody>",
            );
            for row in &rows {
                let sev_class = match row.severity.as_str() {
                    "critical" => "status-down",
                    "error" => "status-degraded",
                    "warning" => "status-degraded",
                    _ => "",
                };
                let ts = &row.last_seen[..19.min(row.last_seen.len())];
                let retry = if row.retryable { "yes" } else { "no" };
                html.push_str(&format!(
                    r#"<tr>
                        <td><code>{}</code></td>
                        <td title="{}">{}</td>
                        <td class="{sev_class}">{}</td>
                        <td>{}</td>
                        <td>{}</td>
                        <td><code>{ts}</code></td>
                        <td>{retry}</td>
                    </tr>"#,
                    html_escape(&row.code),
                    html_escape(&row.suggestion),
                    html_escape(&row.message),
                    row.severity,
                    row.service,
                    row.count,
                ));
            }
            html.push_str("</tbody></table>");
            Html(html)
        }
        Err(e) => Html(format!("<p class='status-down'>Query error: {e}</p>")),
    }
}

/// HTMX fragment: recent individual error instances.
pub async fn errors_recent_panel(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ErrorQuery>,
) -> Html<String> {
    let db = match &state.log_db {
        Some(db) => db,
        None => return Html("<p class='status-down'>Log database not available</p>".into()),
    };

    let conn = db.lock().unwrap();
    let limit = q.limit.unwrap_or(30).min(100);

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref svc) = q.service {
        if !svc.is_empty() {
            conditions.push(format!("service = ?{}", params.len() + 1));
            params.push(Box::new(svc.clone()));
        }
    }
    if let Some(ref code) = q.code {
        if !code.is_empty() {
            conditions.push(format!("code LIKE ?{}", params.len() + 1));
            params.push(Box::new(format!("{code}%")));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT instance_id, code, message, detail, severity, service, suggestion, timestamp
         FROM _error_reports {where_clause}
         ORDER BY timestamp DESC LIMIT {limit}"
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let result = conn.prepare(&sql).and_then(|mut stmt| {
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(ErrorRow {
                instance_id: row.get(0)?,
                code: row.get(1)?,
                message: row.get(2)?,
                detail: row.get(3)?,
                severity: row.get(4)?,
                service: row.get(5)?,
                suggestion: row.get(6)?,
                timestamp: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    });

    match result {
        Ok(rows) => {
            if rows.is_empty() {
                return Html("<p class='loading'>No recent errors</p>".into());
            }
            let mut html = String::from(
                "<table><thead><tr><th>Time</th><th>Code</th><th>Service</th><th>Message</th><th>Detail</th><th>Suggestion</th></tr></thead><tbody>",
            );
            for row in &rows {
                let sev_class = match row.severity.as_str() {
                    "critical" => "status-down",
                    "error" => "status-degraded",
                    _ => "",
                };
                let ts = &row.timestamp[..19.min(row.timestamp.len())];
                let detail = row.detail.as_deref().unwrap_or("-");
                html.push_str(&format!(
                    r#"<tr>
                        <td><code>{ts}</code></td>
                        <td class="{sev_class}"><code>{}</code></td>
                        <td>{}</td>
                        <td>{}</td>
                        <td>{}</td>
                        <td class="suggestion">{}</td>
                    </tr>"#,
                    html_escape(&row.code),
                    html_escape(&row.service),
                    html_escape(&row.message),
                    html_escape(detail),
                    html_escape(&row.suggestion),
                ));
            }
            html.push_str("</tbody></table>");
            Html(html)
        }
        Err(e) => Html(format!("<p class='status-down'>Query error: {e}</p>")),
    }
}

struct ErrorSummary {
    code: String,
    message: String,
    severity: String,
    service: String,
    suggestion: String,
    count: i64,
    last_seen: String,
    retryable: bool,
}

struct ErrorRow {
    #[allow(dead_code)]
    instance_id: String,
    code: String,
    message: String,
    detail: Option<String>,
    severity: String,
    service: String,
    suggestion: String,
    timestamp: String,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
