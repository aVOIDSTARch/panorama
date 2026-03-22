use gateway::logging::audit::{AuditEvent, AuditLogger};
use gateway::logging::operational::{OperationalLogRecord, OperationalLogger};

fn make_operational_logger() -> OperationalLogger {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/001_operational_log.sql"))
        .unwrap();
    OperationalLogger::new(conn)
}

fn make_audit_logger() -> AuditLogger {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/002_audit_log.sql"))
        .unwrap();
    AuditLogger::new(conn)
}

fn sample_op_record(request_id: &str, caller_id: &str, route_key: &str, outcome: &str) -> OperationalLogRecord {
    OperationalLogRecord {
        request_id: request_id.into(),
        caller_id: caller_id.into(),
        route_key: route_key.into(),
        route_key_used: Some(route_key.into()),
        input_tokens: Some(100),
        output_tokens: Some(50),
        total_tokens: Some(150),
        cost_usd: Some(0.001),
        latency_ms: Some(200),
        outcome: outcome.into(),
        error_code: None,
        error_detail: None,
        fallback_used: false,
        fallback_attempt: 0,
        inbound_hash: Some("abc123".into()),
        timestamp: "2026-03-21T10:00:00Z".into(),
        tags: None,
    }
}

// ── Operational Logger ─────────────────────────────────────────────────

#[test]
fn operational_log_insert_and_search_all() {
    let logger = make_operational_logger();
    logger.log_request(&sample_op_record("req-1", "caller-1", "route-1", "success")).unwrap();
    logger.log_request(&sample_op_record("req-2", "caller-2", "route-1", "provider_error")).unwrap();

    let results = logger.search(None, None, None, None, None, None, 100).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn operational_log_search_by_caller() {
    let logger = make_operational_logger();
    logger.log_request(&sample_op_record("req-1", "caller-1", "route-1", "success")).unwrap();
    logger.log_request(&sample_op_record("req-2", "caller-2", "route-1", "success")).unwrap();

    let results = logger.search(Some("caller-1"), None, None, None, None, None, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].caller_id, "caller-1");
}

#[test]
fn operational_log_search_by_route() {
    let logger = make_operational_logger();
    logger.log_request(&sample_op_record("req-1", "c1", "route-a", "success")).unwrap();
    logger.log_request(&sample_op_record("req-2", "c1", "route-b", "success")).unwrap();

    let results = logger.search(None, Some("route-a"), None, None, None, None, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].route_key, "route-a");
}

#[test]
fn operational_log_search_by_outcome() {
    let logger = make_operational_logger();
    logger.log_request(&sample_op_record("req-1", "c1", "r1", "success")).unwrap();
    logger.log_request(&sample_op_record("req-2", "c1", "r1", "provider_error")).unwrap();

    let results = logger.search(None, None, Some("provider_error"), None, None, None, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].outcome, "provider_error");
}

#[test]
fn operational_log_search_by_request_id() {
    let logger = make_operational_logger();
    logger.log_request(&sample_op_record("req-abc", "c1", "r1", "success")).unwrap();
    logger.log_request(&sample_op_record("req-xyz", "c1", "r1", "success")).unwrap();

    let results = logger.search(None, None, None, Some("req-abc"), None, None, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].request_id, "req-abc");
}

#[test]
fn operational_log_search_limit() {
    let logger = make_operational_logger();
    for i in 0..10 {
        logger.log_request(&sample_op_record(&format!("req-{i}"), "c1", "r1", "success")).unwrap();
    }

    let results = logger.search(None, None, None, None, None, None, 3).unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn operational_log_error_fields() {
    let logger = make_operational_logger();
    let mut record = sample_op_record("req-err", "c1", "r1", "provider_error");
    record.error_code = Some("502".into());
    record.error_detail = Some("upstream timeout".into());
    record.fallback_used = true;
    record.fallback_attempt = 1;
    logger.log_request(&record).unwrap();

    let results = logger.search(None, None, None, Some("req-err"), None, None, 1).unwrap();
    assert_eq!(results[0].error_code.as_deref(), Some("502"));
    assert_eq!(results[0].error_detail.as_deref(), Some("upstream timeout"));
    assert!(results[0].fallback_used);
    assert_eq!(results[0].fallback_attempt, 1);
}

// ── Audit Logger ───────────────────────────────────────────────────────

#[test]
fn audit_log_insert_and_search() {
    let logger = make_audit_logger();
    logger.log_event(&AuditEvent {
        event_type: "injection_rejected".into(),
        request_id: Some("req-1".into()),
        caller_id: Some("caller-1".into()),
        route_key: Some("route-1".into()),
        severity: "warn".into(),
        detail: "injection pattern detected".into(),
        timestamp: "2026-03-21T10:00:00Z".into(),
    }).unwrap();

    let results = logger.search(None, None, None, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].event_type, "injection_rejected");
}

#[test]
fn audit_log_search_by_event_type() {
    let logger = make_audit_logger();
    logger.log_event(&AuditEvent {
        event_type: "injection_rejected".into(),
        request_id: None,
        caller_id: None,
        route_key: None,
        severity: "warn".into(),
        detail: "bad input".into(),
        timestamp: "2026-03-21T10:00:00Z".into(),
    }).unwrap();
    logger.log_event(&AuditEvent {
        event_type: "credential_scrub".into(),
        request_id: None,
        caller_id: None,
        route_key: None,
        severity: "critical".into(),
        detail: "api key in response".into(),
        timestamp: "2026-03-21T10:01:00Z".into(),
    }).unwrap();

    let results = logger.search(Some("credential_scrub"), None, None, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].severity, "critical");
}

#[test]
fn audit_log_search_by_severity() {
    let logger = make_audit_logger();
    logger.log_event(&AuditEvent {
        event_type: "test".into(),
        request_id: None,
        caller_id: None,
        route_key: None,
        severity: "warn".into(),
        detail: "warning event".into(),
        timestamp: "2026-03-21T10:00:00Z".into(),
    }).unwrap();
    logger.log_event(&AuditEvent {
        event_type: "test".into(),
        request_id: None,
        caller_id: None,
        route_key: None,
        severity: "critical".into(),
        detail: "critical event".into(),
        timestamp: "2026-03-21T10:01:00Z".into(),
    }).unwrap();

    let results = logger.search(None, Some("critical"), None, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].detail, "critical event");
}

#[test]
fn audit_log_limit() {
    let logger = make_audit_logger();
    for i in 0..10 {
        logger.log_event(&AuditEvent {
            event_type: "test".into(),
            request_id: None,
            caller_id: None,
            route_key: None,
            severity: "info".into(),
            detail: format!("event {i}"),
            timestamp: format!("2026-03-21T10:{i:02}:00Z"),
        }).unwrap();
    }

    let results = logger.search(None, None, None, 5).unwrap();
    assert_eq!(results.len(), 5);
}
