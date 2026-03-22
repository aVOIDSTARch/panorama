CREATE TABLE IF NOT EXISTS operational_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id      TEXT NOT NULL,
    caller_id       TEXT NOT NULL,
    route_key       TEXT NOT NULL,
    route_key_used  TEXT,
    input_tokens    INTEGER,
    output_tokens   INTEGER,
    total_tokens    INTEGER,
    cost_usd        REAL,
    latency_ms      INTEGER,
    outcome         TEXT NOT NULL,
    error_code      TEXT,
    error_detail    TEXT,
    fallback_used   INTEGER DEFAULT 0,
    fallback_attempt INTEGER DEFAULT 0,
    inbound_hash    TEXT,
    timestamp       TEXT NOT NULL,
    tags            TEXT
);

CREATE INDEX IF NOT EXISTS idx_op_timestamp   ON operational_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_op_caller_id   ON operational_log(caller_id);
CREATE INDEX IF NOT EXISTS idx_op_route_key   ON operational_log(route_key);
CREATE INDEX IF NOT EXISTS idx_op_request_id  ON operational_log(request_id);
CREATE INDEX IF NOT EXISTS idx_op_outcome     ON operational_log(outcome);
