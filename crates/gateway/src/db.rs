use rusqlite::Connection;
use std::path::Path;

const MIGRATION_OPERATIONAL: &str = include_str!("../migrations/001_operational_log.sql");
const MIGRATION_AUDIT: &str = include_str!("../migrations/002_audit_log.sql");
const MIGRATION_ROUTE_STORE: &str = include_str!("../migrations/003_route_store.sql");

fn enable_wal(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

fn ensure_migrations_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id      INTEGER PRIMARY KEY,
            name    TEXT NOT NULL UNIQUE,
            applied TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}

fn apply_migration(conn: &Connection, name: &str, sql: &str) -> rusqlite::Result<bool> {
    let already: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM _migrations WHERE name = ?1",
        [name],
        |row| row.get(0),
    )?;
    if already {
        return Ok(false);
    }
    conn.execute_batch(sql)?;
    conn.execute("INSERT INTO _migrations (name) VALUES (?1)", [name])?;
    Ok(true)
}

pub fn init_operational_db(path: &Path) -> anyhow::Result<Connection> {
    ensure_parent_dir(path)?;
    let conn = Connection::open(path)?;
    enable_wal(&conn)?;
    ensure_migrations_table(&conn)?;
    if apply_migration(&conn, "001_operational_log", MIGRATION_OPERATIONAL)? {
        tracing::info!("applied migration 001_operational_log");
    }
    Ok(conn)
}

pub fn init_audit_db(path: &Path) -> anyhow::Result<Connection> {
    ensure_parent_dir(path)?;
    let conn = Connection::open(path)?;
    enable_wal(&conn)?;
    ensure_migrations_table(&conn)?;
    if apply_migration(&conn, "002_audit_log", MIGRATION_AUDIT)? {
        tracing::info!("applied migration 002_audit_log");
    }
    Ok(conn)
}

pub fn init_route_store_db(path: &Path) -> anyhow::Result<Connection> {
    ensure_parent_dir(path)?;
    let conn = Connection::open(path)?;
    enable_wal(&conn)?;
    ensure_migrations_table(&conn)?;
    if apply_migration(&conn, "003_route_store", MIGRATION_ROUTE_STORE)? {
        tracing::info!("applied migration 003_route_store");
    }
    Ok(conn)
}

/// For in-memory testing: initialize all route-store tables on an in-memory connection.
pub fn init_route_store_in_memory() -> anyhow::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    ensure_migrations_table(&conn)?;
    apply_migration(&conn, "003_route_store", MIGRATION_ROUTE_STORE)?;
    Ok(conn)
}

fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
