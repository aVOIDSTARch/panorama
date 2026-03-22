CREATE TABLE IF NOT EXISTS audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type      TEXT NOT NULL,
    request_id      TEXT,
    caller_id       TEXT,
    route_key       TEXT,
    severity        TEXT NOT NULL,
    detail          TEXT NOT NULL,
    timestamp       TEXT NOT NULL
);
