use chrono::Utc;

// We need to access the gateway crate's modules
// Since this is a binary crate, we test via the CLI or use the modules directly
// For unit-style integration tests, we'll test the DB layer directly

#[test]
fn test_route_store_crud() {
    // Initialize in-memory DB with route store schema
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/003_route_store.sql")).unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id      INTEGER PRIMARY KEY,
            name    TEXT NOT NULL UNIQUE,
            applied TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    ).unwrap();

    // Insert a route
    conn.execute(
        "INSERT INTO routes (route_key, display_name, provider, model_id, endpoint_url,
            api_key_env, max_input_tokens, max_output_tokens, cost_per_input_token_usd,
            cost_per_output_token_usd, fallback_chain, health_probe_interval_secs,
            active, version, tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        rusqlite::params![
            "claude-sonnet",
            "Claude Sonnet",
            r#"{"type":"Anthropic"}"#,
            "claude-sonnet-4-20250514",
            "https://api.anthropic.com",
            "ANTHROPIC_API_KEY",
            200000,
            8192,
            0.000003,
            0.000015,
            r#"["claude-haiku"]"#,
            300,
            1,
            1,
            r#"["default"]"#,
        ],
    ).unwrap();

    // Verify it's there
    let count: i32 = conn.query_row("SELECT COUNT(*) FROM routes", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 1);

    // Verify fields
    let display_name: String = conn.query_row(
        "SELECT display_name FROM routes WHERE route_key = 'claude-sonnet'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(display_name, "Claude Sonnet");

    // Update with versioning
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
         FROM routes WHERE route_key = 'claude-sonnet'",
        [],
    ).unwrap();

    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE routes SET max_output_tokens = 16384, version = version + 1, updated_at = ?1 WHERE route_key = 'claude-sonnet'",
        [&now],
    ).unwrap();

    // Verify update
    let new_max: u32 = conn.query_row(
        "SELECT max_output_tokens FROM routes WHERE route_key = 'claude-sonnet'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(new_max, 16384);

    // Verify version incremented
    let version: u32 = conn.query_row(
        "SELECT version FROM routes WHERE route_key = 'claude-sonnet'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(version, 2);

    // Verify history has the old version
    let history_count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM routes_history WHERE route_key = 'claude-sonnet'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(history_count, 1);

    // Disable route
    conn.execute(
        "UPDATE routes SET active = 0, version = version + 1 WHERE route_key = 'claude-sonnet'",
        [],
    ).unwrap();

    let active: bool = conn.query_row(
        "SELECT active FROM routes WHERE route_key = 'claude-sonnet'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert!(!active);
}

#[test]
fn test_caller_tokens() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/003_route_store.sql")).unwrap();

    // Issue a token (store hash)
    let token = "test-token-12345";
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    conn.execute(
        "INSERT INTO caller_tokens (caller_id, token_hash, allowed_routes, active)
         VALUES (?1, ?2, ?3, 1)",
        rusqlite::params!["test-caller", hash, r#"["*"]"#],
    ).unwrap();

    // Validate token
    let stored_hash: String = conn.query_row(
        "SELECT token_hash FROM caller_tokens WHERE caller_id = 'test-caller'",
        [],
        |row| row.get(0),
    ).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let computed = format!("{:x}", hasher.finalize());
    assert_eq!(stored_hash, computed);

    // Revoke
    conn.execute(
        "UPDATE caller_tokens SET active = 0 WHERE caller_id = 'test-caller'",
        [],
    ).unwrap();

    let active: bool = conn.query_row(
        "SELECT active FROM caller_tokens WHERE caller_id = 'test-caller'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert!(!active);
}

#[test]
fn test_operational_log() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/001_operational_log.sql")).unwrap();

    // Insert a log entry
    conn.execute(
        "INSERT INTO operational_log
            (request_id, caller_id, route_key, outcome, timestamp)
         VALUES ('req-1', 'caller-1', 'claude-sonnet', 'success', datetime('now'))",
        [],
    ).unwrap();

    let count: i32 = conn.query_row("SELECT COUNT(*) FROM operational_log", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 1);

    // Insert a failed entry
    conn.execute(
        "INSERT INTO operational_log
            (request_id, caller_id, route_key, outcome, error_code, error_detail, timestamp)
         VALUES ('req-2', 'caller-1', 'gpt4o', 'provider_error', '502', 'timeout', datetime('now'))",
        [],
    ).unwrap();

    // Query by outcome
    let error_count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM operational_log WHERE outcome = 'provider_error'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(error_count, 1);
}
