use chrono::Utc;
use gateway::accounting::budget::{BudgetEnforcer, BudgetExceeded};
use gateway::accounting::cost::CostAccountant;
use gateway::config::BudgetsConfig;
use gateway::types::CostRecord;
use uuid::Uuid;

fn make_cost_accountant() -> CostAccountant {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../migrations/003_route_store.sql"))
        .unwrap();
    CostAccountant::new(conn)
}

fn make_cost_record(caller_id: &str, route_key: &str, cost: f64) -> CostRecord {
    CostRecord {
        request_id: Uuid::new_v4(),
        caller_id: caller_id.into(),
        route_key: route_key.into(),
        route_key_used: route_key.into(),
        input_tokens: 100,
        output_tokens: 50,
        total_tokens: 150,
        estimated_cost_usd: cost,
        fallback_triggered: false,
        fallback_attempt: 0,
        timestamp: Utc::now(),
        outcome: "success".into(),
    }
}

// ── Cost Accountant ────────────────────────────────────────────────────

#[test]
fn record_and_query_caller_spend() {
    let accountant = make_cost_accountant();
    accountant
        .record_cost(&make_cost_record("caller-1", "route-1", 0.50))
        .unwrap();
    accountant
        .record_cost(&make_cost_record("caller-1", "route-2", 0.30))
        .unwrap();

    let spend = accountant.caller_spend_24h("caller-1").unwrap();
    assert!((spend - 0.80).abs() < 0.001);
}

#[test]
fn record_and_query_route_spend() {
    let accountant = make_cost_accountant();
    accountant
        .record_cost(&make_cost_record("caller-1", "route-1", 0.40))
        .unwrap();
    accountant
        .record_cost(&make_cost_record("caller-2", "route-1", 0.60))
        .unwrap();

    let spend = accountant.route_spend_24h("route-1").unwrap();
    assert!((spend - 1.00).abs() < 0.001);
}

#[test]
fn global_spend_sums_all() {
    let accountant = make_cost_accountant();
    accountant
        .record_cost(&make_cost_record("c1", "r1", 1.0))
        .unwrap();
    accountant
        .record_cost(&make_cost_record("c2", "r2", 2.0))
        .unwrap();

    let spend = accountant.global_spend_24h().unwrap();
    assert!((spend - 3.0).abs() < 0.001);
}

#[test]
fn zero_spend_when_no_records() {
    let accountant = make_cost_accountant();
    assert_eq!(accountant.caller_spend_24h("nobody").unwrap(), 0.0);
    assert_eq!(accountant.route_spend_24h("nowhere").unwrap(), 0.0);
    assert_eq!(accountant.global_spend_24h().unwrap(), 0.0);
}

// ── Budget Enforcer ────────────────────────────────────────────────────

#[test]
fn budget_within_limits_passes() {
    let accountant = make_cost_accountant();
    accountant
        .record_cost(&make_cost_record("caller-1", "route-1", 1.0))
        .unwrap();

    let enforcer = BudgetEnforcer::new(BudgetsConfig {
        global_daily_usd: 100.0,
        per_caller_daily_usd: 50.0,
        per_route_daily_usd: 50.0,
    });

    assert!(enforcer.check(&accountant, "caller-1", "route-1").is_ok());
}

#[test]
fn budget_per_caller_exceeded() {
    let accountant = make_cost_accountant();
    accountant
        .record_cost(&make_cost_record("caller-1", "route-1", 5.0))
        .unwrap();

    let enforcer = BudgetEnforcer::new(BudgetsConfig {
        global_daily_usd: 100.0,
        per_caller_daily_usd: 5.0,
        per_route_daily_usd: 100.0,
    });

    let err = enforcer
        .check(&accountant, "caller-1", "route-1")
        .unwrap_err();
    assert!(matches!(err, BudgetExceeded::PerCaller { .. }));
}

#[test]
fn budget_per_route_exceeded() {
    let accountant = make_cost_accountant();
    accountant
        .record_cost(&make_cost_record("caller-1", "route-1", 10.0))
        .unwrap();

    let enforcer = BudgetEnforcer::new(BudgetsConfig {
        global_daily_usd: 100.0,
        per_caller_daily_usd: 100.0,
        per_route_daily_usd: 10.0,
    });

    let err = enforcer
        .check(&accountant, "caller-1", "route-1")
        .unwrap_err();
    assert!(matches!(err, BudgetExceeded::PerRoute { .. }));
}

#[test]
fn budget_global_exceeded() {
    let accountant = make_cost_accountant();
    accountant
        .record_cost(&make_cost_record("c1", "r1", 50.0))
        .unwrap();
    accountant
        .record_cost(&make_cost_record("c2", "r2", 60.0))
        .unwrap();

    let enforcer = BudgetEnforcer::new(BudgetsConfig {
        global_daily_usd: 100.0,
        per_caller_daily_usd: 200.0,
        per_route_daily_usd: 200.0,
    });

    let err = enforcer.check(&accountant, "c1", "r1").unwrap_err();
    assert!(matches!(err, BudgetExceeded::Global { .. }));
}

#[test]
fn budget_display_format() {
    let exceeded = BudgetExceeded::PerCaller {
        caller_id: "alice".into(),
        spend: 5.1234,
        ceiling: 5.0,
    };
    let msg = exceeded.to_string();
    assert!(msg.contains("alice"));
    assert!(msg.contains("5.12")); // spend formatted
    assert!(msg.contains("5.00")); // ceiling formatted
}
