use rusqlite::Connection;
use std::sync::Mutex;

pub struct AuditLogger {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub event_type: String,
    pub request_id: Option<String>,
    pub caller_id: Option<String>,
    pub route_key: Option<String>,
    pub severity: String,
    pub detail: String,
    pub timestamp: String,
}

impl AuditLogger {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn log_event(&self, event: &AuditEvent) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_log (event_type, request_id, caller_id, route_key, severity, detail, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                event.event_type,
                event.request_id,
                event.caller_id,
                event.route_key,
                event.severity,
                event.detail,
                event.timestamp,
            ],
        )?;
        Ok(())
    }

    pub fn search(
        &self,
        event_type: Option<&str>,
        severity: Option<&str>,
        from: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<AuditEvent>> {
        let conn = self.conn.lock().unwrap();

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(et) = event_type {
            conditions.push(format!("event_type = ?{}", params.len() + 1));
            params.push(Box::new(et.to_string()));
        }
        if let Some(s) = severity {
            conditions.push(format!("severity = ?{}", params.len() + 1));
            params.push(Box::new(s.to_string()));
        }
        if let Some(f) = from {
            conditions.push(format!("timestamp >= ?{}", params.len() + 1));
            params.push(Box::new(f.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT event_type, request_id, caller_id, route_key, severity, detail, timestamp
             FROM audit_log {where_clause}
             ORDER BY id DESC LIMIT {limit}"
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(AuditEvent {
                event_type: row.get(0)?,
                request_id: row.get(1)?,
                caller_id: row.get(2)?,
                route_key: row.get(3)?,
                severity: row.get(4)?,
                detail: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }
}
