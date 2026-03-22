use crate::types::CostRecord;
use rusqlite::Connection;
use std::sync::Mutex;

pub struct CostAccountant {
    conn: Mutex<Connection>,
}

impl CostAccountant {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn record_cost(&self, record: &CostRecord) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO cost_records
                (request_id, caller_id, route_key, route_key_used, input_tokens, output_tokens,
                 total_tokens, estimated_cost_usd, fallback_triggered, fallback_attempt,
                 timestamp, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                record.request_id.to_string(),
                record.caller_id,
                record.route_key,
                record.route_key_used,
                record.input_tokens,
                record.output_tokens,
                record.total_tokens,
                record.estimated_cost_usd,
                record.fallback_triggered as i32,
                record.fallback_attempt as i32,
                record.timestamp.to_rfc3339(),
                record.outcome,
            ],
        )?;
        Ok(())
    }

    pub fn caller_spend_24h(&self, caller_id: &str) -> anyhow::Result<f64> {
        let conn = self.conn.lock().unwrap();
        let spend: f64 = conn.query_row(
            "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) FROM cost_records
             WHERE caller_id = ?1 AND timestamp >= datetime('now', '-1 day') AND is_probe = 0",
            [caller_id],
            |row| row.get(0),
        )?;
        Ok(spend)
    }

    pub fn route_spend_24h(&self, route_key: &str) -> anyhow::Result<f64> {
        let conn = self.conn.lock().unwrap();
        let spend: f64 = conn.query_row(
            "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) FROM cost_records
             WHERE route_key = ?1 AND timestamp >= datetime('now', '-1 day') AND is_probe = 0",
            [route_key],
            |row| row.get(0),
        )?;
        Ok(spend)
    }

    pub fn global_spend_24h(&self) -> anyhow::Result<f64> {
        let conn = self.conn.lock().unwrap();
        let spend: f64 = conn.query_row(
            "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) FROM cost_records
             WHERE timestamp >= datetime('now', '-1 day') AND is_probe = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(spend)
    }
}
