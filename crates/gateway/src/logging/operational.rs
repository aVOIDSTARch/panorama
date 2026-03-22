use rusqlite::Connection;
use std::sync::Mutex;

pub struct OperationalLogger {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct OperationalLogRecord {
    pub request_id: String,
    pub caller_id: String,
    pub route_key: String,
    pub route_key_used: Option<String>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cost_usd: Option<f64>,
    pub latency_ms: Option<u64>,
    pub outcome: String,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub fallback_used: bool,
    pub fallback_attempt: u8,
    pub inbound_hash: Option<String>,
    pub timestamp: String,
    pub tags: Option<String>,
}

impl OperationalLogger {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn log_request(&self, record: &OperationalLogRecord) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO operational_log
                (request_id, caller_id, route_key, route_key_used, input_tokens, output_tokens,
                 total_tokens, cost_usd, latency_ms, outcome, error_code, error_detail,
                 fallback_used, fallback_attempt, inbound_hash, timestamp, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            rusqlite::params![
                record.request_id,
                record.caller_id,
                record.route_key,
                record.route_key_used,
                record.input_tokens,
                record.output_tokens,
                record.total_tokens,
                record.cost_usd,
                record.latency_ms.map(|v| v as i64),
                record.outcome,
                record.error_code,
                record.error_detail,
                record.fallback_used as i32,
                record.fallback_attempt as i32,
                record.inbound_hash,
                record.timestamp,
                record.tags,
            ],
        )?;
        Ok(())
    }

    pub fn search(
        &self,
        caller: Option<&str>,
        route: Option<&str>,
        outcome: Option<&str>,
        request_id: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<OperationalLogRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(c) = caller {
            conditions.push(format!("caller_id = ?{}", params.len() + 1));
            params.push(Box::new(c.to_string()));
        }
        if let Some(r) = route {
            conditions.push(format!("route_key = ?{}", params.len() + 1));
            params.push(Box::new(r.to_string()));
        }
        if let Some(o) = outcome {
            conditions.push(format!("outcome = ?{}", params.len() + 1));
            params.push(Box::new(o.to_string()));
        }
        if let Some(rid) = request_id {
            conditions.push(format!("request_id = ?{}", params.len() + 1));
            params.push(Box::new(rid.to_string()));
        }
        if let Some(f) = from {
            conditions.push(format!("timestamp >= ?{}", params.len() + 1));
            params.push(Box::new(f.to_string()));
        }
        if let Some(t) = to {
            conditions.push(format!("timestamp <= ?{}", params.len() + 1));
            params.push(Box::new(t.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT request_id, caller_id, route_key, route_key_used, input_tokens, output_tokens,
                    total_tokens, cost_usd, latency_ms, outcome, error_code, error_detail,
                    fallback_used, fallback_attempt, inbound_hash, timestamp, tags
             FROM operational_log {where_clause}
             ORDER BY timestamp DESC LIMIT {limit}"
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(OperationalLogRecord {
                request_id: row.get(0)?,
                caller_id: row.get(1)?,
                route_key: row.get(2)?,
                route_key_used: row.get(3)?,
                input_tokens: row.get(4)?,
                output_tokens: row.get(5)?,
                total_tokens: row.get(6)?,
                cost_usd: row.get(7)?,
                latency_ms: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
                outcome: row.get(9)?,
                error_code: row.get(10)?,
                error_detail: row.get(11)?,
                fallback_used: row.get::<_, i32>(12)? != 0,
                fallback_attempt: row.get::<_, i32>(13)? as u8,
                inbound_hash: row.get(14)?,
                timestamp: row.get(15)?,
                tags: row.get(16)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }
}
