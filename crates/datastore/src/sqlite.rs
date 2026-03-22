use rusqlite::{params, Connection};
use std::sync::Mutex;

/// SQLite storage backend (WAL mode) for structured data.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        // Bootstrap the meta table (tracks user-created tables)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _datastore_tables (
                name TEXT PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                schema_json TEXT
            )",
            [],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// List all user-created tables.
    pub fn list_tables(&self) -> anyhow::Result<Vec<TableInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, created_at, schema_json FROM _datastore_tables ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TableInfo {
                name: row.get(0)?,
                created_at: row.get(1)?,
                schema_json: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Create a table following the Datastore pattern (id, created_at, updated_at, meta, + custom columns).
    pub fn create_table(&self, name: &str, columns: &[(String, String)]) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut col_defs = vec![
            "id TEXT PRIMARY KEY".to_string(),
            "created_at TEXT NOT NULL DEFAULT (datetime('now'))".to_string(),
            "updated_at TEXT NOT NULL DEFAULT (datetime('now'))".to_string(),
            "meta TEXT DEFAULT '{}'".to_string(),
        ];
        for (col_name, col_type) in columns {
            col_defs.push(format!("{col_name} {col_type}"));
        }
        let sql = format!("CREATE TABLE IF NOT EXISTS [{name}] ({})", col_defs.join(", "));
        conn.execute(&sql, [])?;

        let schema = serde_json::to_string(columns)?;
        conn.execute(
            "INSERT OR REPLACE INTO _datastore_tables (name, schema_json) VALUES (?1, ?2)",
            params![name, schema],
        )?;
        Ok(())
    }

    /// List rows from a table with optional filters.
    pub fn list_objects(
        &self,
        table: &str,
        limit: u32,
        offset: u32,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "SELECT * FROM [{table}] ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn.prepare(&sql)?;
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap().to_string())
            .collect();

        let rows = stmt.query_map(params![limit, offset], |row| {
            let mut obj = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val: String = row.get(i).unwrap_or_default();
                obj.insert(name.clone(), serde_json::Value::String(val));
            }
            Ok(serde_json::Value::Object(obj))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get a single row by ID.
    pub fn get_object(
        &self,
        table: &str,
        id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("SELECT * FROM [{table}] WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap().to_string())
            .collect();

        let result = stmt.query_row(params![id], |row| {
            let mut obj = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val: String = row.get(i).unwrap_or_default();
                obj.insert(name.clone(), serde_json::Value::String(val));
            }
            Ok(serde_json::Value::Object(obj))
        });

        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Insert a row. Expects a JSON object with field names matching columns.
    pub fn insert_object(
        &self,
        table: &str,
        data: &serde_json::Value,
    ) -> anyhow::Result<String> {
        let obj = data
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Expected JSON object"))?;

        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let mut cols = vec!["id".to_string()];
        let mut placeholders = vec!["?1".to_string()];
        let mut values: Vec<String> = vec![id.clone()];
        let mut idx = 2;

        for (key, val) in obj {
            if key == "id" {
                continue;
            }
            cols.push(format!("[{key}]"));
            placeholders.push(format!("?{idx}"));
            values.push(val.as_str().unwrap_or(&val.to_string()).to_string());
            idx += 1;
        }

        let sql = format!(
            "INSERT INTO [{table}] ({}) VALUES ({})",
            cols.join(", "),
            placeholders.join(", ")
        );

        let conn = self.conn.lock().unwrap();
        let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        conn.execute(&sql, params.as_slice())?;

        Ok(id)
    }

    /// Delete a row by ID.
    pub fn delete_object(&self, table: &str, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("DELETE FROM [{table}] WHERE id = ?1");
        let affected = conn.execute(&sql, params![id])?;
        Ok(affected > 0)
    }

    /// Execute a named/curated query (Tier 2).
    pub fn execute_query(
        &self,
        sql: &str,
        params_list: &[String],
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql)?;
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap().to_string())
            .collect();

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params_list.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let mut obj = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val: String = row.get(i).unwrap_or_default();
                obj.insert(name.clone(), serde_json::Value::String(val));
            }
            Ok(serde_json::Value::Object(obj))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

pub struct TableInfo {
    pub name: String,
    pub created_at: String,
    pub schema_json: Option<String>,
}
