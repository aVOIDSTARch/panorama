-- Routes table
CREATE TABLE IF NOT EXISTS routes (
    route_key               TEXT PRIMARY KEY,
    display_name            TEXT NOT NULL,
    provider                TEXT NOT NULL,
    model_id                TEXT NOT NULL,
    endpoint_url            TEXT NOT NULL,
    api_key_env             TEXT NOT NULL,
    max_input_tokens        INTEGER NOT NULL DEFAULT 4096,
    max_output_tokens       INTEGER NOT NULL DEFAULT 4096,
    cost_per_input_token_usd  REAL NOT NULL DEFAULT 0.0,
    cost_per_output_token_usd REAL NOT NULL DEFAULT 0.0,
    fallback_chain          TEXT NOT NULL DEFAULT '[]',
    health_probe_interval_secs INTEGER NOT NULL DEFAULT 300,
    active                  INTEGER NOT NULL DEFAULT 1,
    version                 INTEGER NOT NULL DEFAULT 1,
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now')),
    tags                    TEXT NOT NULL DEFAULT '[]'
);

-- Route version history
CREATE TABLE IF NOT EXISTS routes_history (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    route_key               TEXT NOT NULL,
    display_name            TEXT NOT NULL,
    provider                TEXT NOT NULL,
    model_id                TEXT NOT NULL,
    endpoint_url            TEXT NOT NULL,
    api_key_env             TEXT NOT NULL,
    max_input_tokens        INTEGER NOT NULL,
    max_output_tokens       INTEGER NOT NULL,
    cost_per_input_token_usd  REAL NOT NULL,
    cost_per_output_token_usd REAL NOT NULL,
    fallback_chain          TEXT NOT NULL,
    health_probe_interval_secs INTEGER NOT NULL,
    active                  INTEGER NOT NULL,
    version                 INTEGER NOT NULL,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    tags                    TEXT NOT NULL,
    archived_at             TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_rh_route_key ON routes_history(route_key);
CREATE INDEX IF NOT EXISTS idx_rh_version   ON routes_history(route_key, version);

-- Caller tokens
CREATE TABLE IF NOT EXISTS caller_tokens (
    caller_id       TEXT PRIMARY KEY,
    token_hash      TEXT NOT NULL UNIQUE,
    issued_at       TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at      TEXT,
    allowed_routes  TEXT NOT NULL DEFAULT '["*"]',
    active          INTEGER NOT NULL DEFAULT 1
);

-- Cost records
CREATE TABLE IF NOT EXISTS cost_records (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id          TEXT NOT NULL,
    caller_id           TEXT NOT NULL,
    route_key           TEXT NOT NULL,
    route_key_used      TEXT NOT NULL,
    input_tokens        INTEGER NOT NULL DEFAULT 0,
    output_tokens       INTEGER NOT NULL DEFAULT 0,
    total_tokens        INTEGER NOT NULL DEFAULT 0,
    estimated_cost_usd  REAL NOT NULL DEFAULT 0.0,
    fallback_triggered  INTEGER NOT NULL DEFAULT 0,
    fallback_attempt    INTEGER NOT NULL DEFAULT 0,
    timestamp           TEXT NOT NULL DEFAULT (datetime('now')),
    outcome             TEXT NOT NULL,
    is_probe            INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_cr_caller_id  ON cost_records(caller_id);
CREATE INDEX IF NOT EXISTS idx_cr_route_key  ON cost_records(route_key);
CREATE INDEX IF NOT EXISTS idx_cr_timestamp  ON cost_records(timestamp);
CREATE INDEX IF NOT EXISTS idx_cr_request_id ON cost_records(request_id);
