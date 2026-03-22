use crate::types::{Provider, Route};
use chrono::Utc;
use rusqlite::Connection;
use std::sync::Mutex;

pub struct RouteStore {
    conn: Mutex<Connection>,
}

impl RouteStore {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn list_routes(&self) -> anyhow::Result<Vec<Route>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT route_key, display_name, provider, model_id, endpoint_url, api_key_env,
                    max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                    cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                    active, version, created_at, updated_at, tags
             FROM routes ORDER BY route_key",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(RouteRow {
                route_key: row.get(0)?,
                display_name: row.get(1)?,
                provider: row.get(2)?,
                model_id: row.get(3)?,
                endpoint_url: row.get(4)?,
                api_key_env: row.get(5)?,
                max_input_tokens: row.get(6)?,
                max_output_tokens: row.get(7)?,
                cost_per_input_token_usd: row.get(8)?,
                cost_per_output_token_usd: row.get(9)?,
                fallback_chain: row.get(10)?,
                health_probe_interval_secs: row.get(11)?,
                active: row.get(12)?,
                version: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                tags: row.get(16)?,
            })
        })?;

        let mut routes = Vec::new();
        for row in rows {
            routes.push(row_to_route(row?)?);
        }
        Ok(routes)
    }

    pub fn get_route(&self, route_key: &str) -> anyhow::Result<Option<Route>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT route_key, display_name, provider, model_id, endpoint_url, api_key_env,
                    max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                    cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                    active, version, created_at, updated_at, tags
             FROM routes WHERE route_key = ?1",
        )?;
        let mut rows = stmt.query_map([route_key], |row| {
            Ok(RouteRow {
                route_key: row.get(0)?,
                display_name: row.get(1)?,
                provider: row.get(2)?,
                model_id: row.get(3)?,
                endpoint_url: row.get(4)?,
                api_key_env: row.get(5)?,
                max_input_tokens: row.get(6)?,
                max_output_tokens: row.get(7)?,
                cost_per_input_token_usd: row.get(8)?,
                cost_per_output_token_usd: row.get(9)?,
                fallback_chain: row.get(10)?,
                health_probe_interval_secs: row.get(11)?,
                active: row.get(12)?,
                version: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                tags: row.get(16)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row_to_route(row?)?)),
            None => Ok(None),
        }
    }

    pub fn add_route(&self, route: &Route) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let provider_json = serde_json::to_string(&route.provider)?;
        let fallback_json = serde_json::to_string(&route.fallback_chain)?;
        let tags_json = serde_json::to_string(&route.tags)?;

        conn.execute(
            "INSERT INTO routes (route_key, display_name, provider, model_id, endpoint_url,
                api_key_env, max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                active, version, created_at, updated_at, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            rusqlite::params![
                route.route_key,
                route.display_name,
                provider_json,
                route.model_id,
                route.endpoint_url,
                route.api_key_env,
                route.max_input_tokens,
                route.max_output_tokens,
                route.cost_per_input_token_usd,
                route.cost_per_output_token_usd,
                fallback_json,
                route.health_probe_interval_secs,
                route.active,
                route.version,
                route.created_at.to_rfc3339(),
                route.updated_at.to_rfc3339(),
                tags_json,
            ],
        )?;
        Ok(())
    }

    pub fn update_route(&self, route_key: &str, field: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();

        // Archive current version to history
        conn.execute(
            "INSERT INTO routes_history
                (route_key, display_name, provider, model_id, endpoint_url, api_key_env,
                 max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                 cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                 active, version, created_at, updated_at, tags)
             SELECT route_key, display_name, provider, model_id, endpoint_url, api_key_env,
                    max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                    cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                    active, version, created_at, updated_at, tags
             FROM routes WHERE route_key = ?1",
            [route_key],
        )?;

        // Validate field name to prevent SQL injection
        let allowed_fields = [
            "display_name",
            "model_id",
            "endpoint_url",
            "api_key_env",
            "max_input_tokens",
            "max_output_tokens",
            "cost_per_input_token_usd",
            "cost_per_output_token_usd",
            "fallback_chain",
            "health_probe_interval_secs",
            "tags",
        ];
        if !allowed_fields.contains(&field) {
            anyhow::bail!("cannot update field '{field}' — allowed: {allowed_fields:?}");
        }

        let sql = format!(
            "UPDATE routes SET {field} = ?1, version = version + 1, updated_at = ?2 WHERE route_key = ?3"
        );
        let now = Utc::now().to_rfc3339();
        conn.execute(&sql, rusqlite::params![value, now, route_key])?;

        Ok(())
    }

    pub fn disable_route(&self, route_key: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routes SET active = 0, version = version + 1, updated_at = ?1 WHERE route_key = ?2",
            rusqlite::params![now, route_key],
        )?;
        Ok(())
    }

    pub fn enable_route(&self, route_key: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routes SET active = 1, version = version + 1, updated_at = ?1 WHERE route_key = ?2",
            rusqlite::params![now, route_key],
        )?;
        Ok(())
    }

    pub fn route_history(&self, route_key: &str) -> anyhow::Result<Vec<Route>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT route_key, display_name, provider, model_id, endpoint_url, api_key_env,
                    max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                    cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                    active, version, created_at, updated_at, tags
             FROM routes_history WHERE route_key = ?1 ORDER BY version DESC",
        )?;
        let rows = stmt.query_map([route_key], |row| {
            Ok(RouteRow {
                route_key: row.get(0)?,
                display_name: row.get(1)?,
                provider: row.get(2)?,
                model_id: row.get(3)?,
                endpoint_url: row.get(4)?,
                api_key_env: row.get(5)?,
                max_input_tokens: row.get(6)?,
                max_output_tokens: row.get(7)?,
                cost_per_input_token_usd: row.get(8)?,
                cost_per_output_token_usd: row.get(9)?,
                fallback_chain: row.get(10)?,
                health_probe_interval_secs: row.get(11)?,
                active: row.get(12)?,
                version: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                tags: row.get(16)?,
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row_to_route(row?)?);
        }
        Ok(history)
    }

    pub fn rollback_route(&self, route_key: &str, version: u32) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();

        // Archive current to history first
        conn.execute(
            "INSERT INTO routes_history
                (route_key, display_name, provider, model_id, endpoint_url, api_key_env,
                 max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                 cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                 active, version, created_at, updated_at, tags)
             SELECT route_key, display_name, provider, model_id, endpoint_url, api_key_env,
                    max_input_tokens, max_output_tokens, cost_per_input_token_usd,
                    cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
                    active, version, created_at, updated_at, tags
             FROM routes WHERE route_key = ?1",
            [route_key],
        )?;

        // Get the current version number for incrementing
        let current_version: u32 = conn.query_row(
            "SELECT version FROM routes WHERE route_key = ?1",
            [route_key],
            |row| row.get(0),
        )?;

        // Restore from history
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE routes SET
                display_name = h.display_name,
                provider = h.provider,
                model_id = h.model_id,
                endpoint_url = h.endpoint_url,
                api_key_env = h.api_key_env,
                max_input_tokens = h.max_input_tokens,
                max_output_tokens = h.max_output_tokens,
                cost_per_input_token_usd = h.cost_per_input_token_usd,
                cost_per_output_token_usd = h.cost_per_output_token_usd,
                fallback_chain = h.fallback_chain,
                health_probe_interval_secs = h.health_probe_interval_secs,
                active = h.active,
                tags = h.tags,
                version = ?1,
                updated_at = ?2
             FROM routes_history h
             WHERE routes.route_key = ?3
               AND h.route_key = ?3
               AND h.version = ?4",
            rusqlite::params![current_version + 1, now, route_key, version],
        )?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal row type for SQLite mapping
// ---------------------------------------------------------------------------

struct RouteRow {
    route_key: String,
    display_name: String,
    provider: String,
    model_id: String,
    endpoint_url: String,
    api_key_env: String,
    max_input_tokens: u32,
    max_output_tokens: u32,
    cost_per_input_token_usd: f64,
    cost_per_output_token_usd: f64,
    fallback_chain: String,
    health_probe_interval_secs: u64,
    active: bool,
    version: u32,
    created_at: String,
    updated_at: String,
    tags: String,
}

fn row_to_route(row: RouteRow) -> anyhow::Result<Route> {
    let provider: Provider = serde_json::from_str(&row.provider)
        .map_err(|e| anyhow::anyhow!("invalid provider JSON '{}': {e}", row.provider))?;
    let fallback_chain: Vec<String> = serde_json::from_str(&row.fallback_chain)
        .unwrap_or_default();
    let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();

    let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(&row.created_at, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc().fixed_offset())
        })
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let updated_at = chrono::DateTime::parse_from_rfc3339(&row.updated_at)
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(&row.updated_at, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc().fixed_offset())
        })
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    Ok(Route {
        route_key: row.route_key,
        display_name: row.display_name,
        provider,
        model_id: row.model_id,
        endpoint_url: row.endpoint_url,
        api_key_env: row.api_key_env,
        max_input_tokens: row.max_input_tokens,
        max_output_tokens: row.max_output_tokens,
        cost_per_input_token_usd: row.cost_per_input_token_usd,
        cost_per_output_token_usd: row.cost_per_output_token_usd,
        fallback_chain,
        health_probe_interval_secs: row.health_probe_interval_secs,
        active: row.active,
        version: row.version,
        created_at,
        updated_at,
        tags,
    })
}
