use crate::accounting::budget::BudgetEnforcer;
use crate::accounting::cost::CostAccountant;
use crate::config::GatewayConfig;
use crate::dedup::fingerprint::Deduplicator;
use crate::error::GatewayApiError;
use crate::identity::cloak_auth;
use crate::kill_switch::controller::KillSwitchController;
use crate::logging::audit::{AuditEvent, AuditLogger};
use crate::logging::operational::{OperationalLogRecord, OperationalLogger};
use crate::rate_limit::limiter::RateLimiter;
use crate::routes::health::HealthMap;
use crate::routes::store::RouteStore;
use crate::sanitizer::inbound::InboundSanitizer;
use crate::sanitizer::outbound::OutboundSanitizer;
use crate::types::{
    CostRecord, InboundRequest, OutboundResponse, ResponseStatus, UsageSummary,
};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{middleware, Extension, Json, Router};
use chrono::Utc;
use cloak_core::TokenClaims;
use cloak_sdk::CloakState;
use std::sync::Arc;
use std::time::Instant;

pub struct AppState {
    pub config: GatewayConfig,
    pub cloak: CloakState,
    pub route_store: RouteStore,
    pub http_client: reqwest::Client,
    pub operational_logger: OperationalLogger,
    pub audit_logger: AuditLogger,
    pub cost_accountant: CostAccountant,
    pub budget_enforcer: BudgetEnforcer,
    pub rate_limiter: RateLimiter,
    pub deduplicator: Option<Deduplicator>,
    pub kill_switch: KillSwitchController,
    pub health_map: HealthMap,
    pub inbound_sanitizer: InboundSanitizer,
    pub outbound_sanitizer: OutboundSanitizer,
}

pub fn build_request_router(state: Arc<AppState>) -> Router {
    // Authenticated routes — cloak middleware verifies tokens and inserts TokenClaims
    let authed = Router::new()
        .route("/routes", get(list_routes_handler))
        .route("/routes/:route_key", get(get_route_handler))
        .route("/dispatch", post(dispatch_handler))
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::cloak_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::halt_guard,
        ))
        .with_state(state.clone());

    // Health check — no auth required
    let health = Router::new()
        .route("/health", get(health_handler))
        .with_state(state);

    Router::new().merge(health).merge(authed)
}

pub fn build_admin_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/status", get(admin_status_handler))
        .route("/admin/kill", delete(admin_kill_handler))
        .route("/admin/resume", post(admin_resume_handler))
        .route("/admin/probe/:route_key", post(admin_probe_handler))
        .route("/admin/config/reload", post(admin_config_reload_handler))
        .route("/admin/cost/summary", get(admin_cost_summary_handler))
        .route("/admin/budget", get(admin_budget_get_handler))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Request port handlers
// ---------------------------------------------------------------------------

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ks_state = state.kill_switch.state();
    let body = serde_json::json!({
        "status": if state.kill_switch.is_operational() { "ok" } else { "degraded" },
        "kill_switch": ks_state,
        "port": state.config.server.port,
        "admin_port": state.config.server.admin_port,
        "timestamp": Utc::now().to_rfc3339(),
    });
    let status = if state.kill_switch.is_operational() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body))
}

async fn list_routes_handler(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<TokenClaims>,
) -> Result<impl IntoResponse, GatewayApiError> {
    let routes = state
        .route_store
        .list_routes()
        .map_err(|e| GatewayApiError::Internal(e.to_string()))?;

    let summary: Vec<serde_json::Value> = routes
        .iter()
        .map(|r| {
            serde_json::json!({
                "route_key": r.route_key,
                "display_name": r.display_name,
                "provider": r.provider,
                "model_id": r.model_id,
                "active": r.active,
                "version": r.version,
                "max_input_tokens": r.max_input_tokens,
                "max_output_tokens": r.max_output_tokens,
                "fallback_chain": r.fallback_chain,
                "tags": r.tags,
            })
        })
        .collect();

    Ok(Json(summary))
}

async fn get_route_handler(
    State(state): State<Arc<AppState>>,
    Extension(_claims): Extension<TokenClaims>,
    axum::extract::Path(route_key): axum::extract::Path<String>,
) -> Result<impl IntoResponse, GatewayApiError> {
    let route = state
        .route_store
        .get_route(&route_key)
        .map_err(|e| GatewayApiError::Internal(e.to_string()))?
        .ok_or_else(|| GatewayApiError::NotFound(format!("route '{route_key}' not found")))?;

    let body = serde_json::json!({
        "route_key": route.route_key,
        "display_name": route.display_name,
        "provider": route.provider,
        "model_id": route.model_id,
        "endpoint_url": route.endpoint_url,
        "active": route.active,
        "version": route.version,
        "max_input_tokens": route.max_input_tokens,
        "max_output_tokens": route.max_output_tokens,
        "cost_per_input_token_usd": route.cost_per_input_token_usd,
        "cost_per_output_token_usd": route.cost_per_output_token_usd,
        "fallback_chain": route.fallback_chain,
        "health_probe_interval_secs": route.health_probe_interval_secs,
        "tags": route.tags,
        "created_at": route.created_at.to_rfc3339(),
        "updated_at": route.updated_at.to_rfc3339(),
    });

    Ok(Json(body))
}

async fn dispatch_handler(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<TokenClaims>,
    Json(request): Json<InboundRequest>,
) -> Result<impl IntoResponse, GatewayApiError> {
    let start = Instant::now();

    // 1. Identity from Cloak claims (token already verified by middleware)
    let caller = cloak_auth::claims_to_identity(&claims);

    // 2. Route access check
    if !cloak_auth::check_route_access(&caller, &request.route_key) {
        return Err(GatewayApiError::Unauthorized(format!(
            "caller '{}' not authorized for route '{}'",
            caller.caller_id, request.route_key
        )));
    }

    // 3. Kill switch check
    if !state.kill_switch.is_operational() {
        return Err(GatewayApiError::ServiceUnavailable(
            "gateway is in drain/halt mode".into(),
        ));
    }

    // 4. Inbound sanitization
    let sanitized = state.inbound_sanitizer.sanitize(&request).map_err(|e| {
        // Log to audit
        let _ = state.audit_logger.log_event(&AuditEvent {
            event_type: "sanitization_reject".into(),
            request_id: Some(request.request_id.to_string()),
            caller_id: Some(request.caller_metadata.caller_id.clone()),
            route_key: Some(request.route_key.clone()),
            severity: "WARN".into(),
            detail: serde_json::json!({"kind": format!("{:?}", e.kind), "detail": e.detail}).to_string(),
            timestamp: Utc::now().to_rfc3339(),
        });
        GatewayApiError::BadRequest(format!("sanitization failed: {}", e.detail))
    })?;

    // 5. Rate limit check
    if let Err(retry_after) = state.rate_limiter.check(&caller.caller_id, &request.route_key) {
        return Err(GatewayApiError::RateLimited {
            retry_after_secs: retry_after,
        });
    }

    // 6. Deduplication check
    if let Some(ref dedup) = state.deduplicator {
        if let Err(original_id) = dedup.check(&sanitized.inbound_hash, &request.request_id.to_string()) {
            return Err(GatewayApiError::Conflict {
                original_request_id: original_id,
            });
        }
    }

    // 7. Route lookup
    let route = state
        .route_store
        .get_route(&request.route_key)
        .map_err(|e| GatewayApiError::Internal(e.to_string()))?
        .ok_or_else(|| GatewayApiError::NotFound(format!("route '{}' not found", request.route_key)))?;

    if !route.active {
        return Err(GatewayApiError::ServiceUnavailable(format!(
            "route '{}' is inactive",
            request.route_key
        )));
    }

    // 8. Budget check
    if let Err(exceeded) = state.budget_enforcer.check(
        &state.cost_accountant,
        &caller.caller_id,
        &request.route_key,
    ) {
        return Err(GatewayApiError::PaymentRequired(exceeded.to_string()));
    }

    // 9. Dispatch with fallback
    let provider_result = crate::routes::dispatcher::dispatch_with_fallback(
        &state.http_client,
        &state.route_store,
        &state.health_map,
        &sanitized,
    )
    .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match provider_result {
        Ok(provider_resp) => {
            // 10. Outbound sanitization
            let sanitized_response = match state.outbound_sanitizer.sanitize(&provider_resp.raw_response) {
                Ok(text) => text,
                Err(e) => {
                    // Credential scrub hit — CRITICAL
                    state.kill_switch.notify_credential_scrub();
                    let _ = state.audit_logger.log_event(&AuditEvent {
                        event_type: "credential_scrub".into(),
                        request_id: Some(request.request_id.to_string()),
                        caller_id: Some(caller.caller_id.clone()),
                        route_key: Some(provider_resp.route_key.clone()),
                        severity: "CRITICAL".into(),
                        detail: e.detail.clone(),
                        timestamp: Utc::now().to_rfc3339(),
                    });
                    return Err(GatewayApiError::BadGateway(
                        "response rejected: credential pattern detected".into(),
                    ));
                }
            };

            // Reset critical counter on success
            state.kill_switch.reset_criticals();

            let used_route = state.route_store.get_route(&provider_resp.route_key).ok().flatten().unwrap_or(route.clone());
            let cost = (provider_resp.input_tokens as f64 * used_route.cost_per_input_token_usd)
                + (provider_resp.output_tokens as f64 * used_route.cost_per_output_token_usd);

            // 11. Record cost
            let _ = state.cost_accountant.record_cost(&CostRecord {
                request_id: request.request_id,
                caller_id: caller.caller_id.clone(),
                route_key: request.route_key.clone(),
                route_key_used: provider_resp.route_key.clone(),
                input_tokens: provider_resp.input_tokens,
                output_tokens: provider_resp.output_tokens,
                total_tokens: provider_resp.input_tokens + provider_resp.output_tokens,
                estimated_cost_usd: cost,
                fallback_triggered: provider_resp.fallback_attempt > 0,
                fallback_attempt: provider_resp.fallback_attempt,
                timestamp: Utc::now(),
                outcome: "success".into(),
            });

            // 12. Log request
            let _ = state.operational_logger.log_request(&OperationalLogRecord {
                request_id: request.request_id.to_string(),
                caller_id: caller.caller_id,
                route_key: request.route_key.clone(),
                route_key_used: Some(provider_resp.route_key.clone()),
                input_tokens: Some(provider_resp.input_tokens),
                output_tokens: Some(provider_resp.output_tokens),
                total_tokens: Some(provider_resp.input_tokens + provider_resp.output_tokens),
                cost_usd: Some(cost),
                latency_ms: Some(latency_ms),
                outcome: "success".into(),
                error_code: None,
                error_detail: None,
                fallback_used: provider_resp.fallback_attempt > 0,
                fallback_attempt: provider_resp.fallback_attempt,
                inbound_hash: Some(sanitized.inbound_hash),
                timestamp: Utc::now().to_rfc3339(),
                tags: None,
            });

            let response = OutboundResponse {
                request_id: request.request_id,
                status: ResponseStatus::Success,
                route_key_used: provider_resp.route_key,
                fallback_triggered: provider_resp.fallback_attempt > 0,
                response: Some(sanitized_response),
                usage: Some(UsageSummary {
                    input_tokens: provider_resp.input_tokens,
                    output_tokens: provider_resp.output_tokens,
                    total_tokens: provider_resp.input_tokens + provider_resp.output_tokens,
                    estimated_cost_usd: cost,
                }),
                error: None,
                latency_ms,
                timestamp: Utc::now(),
            };

            Ok(Json(response))
        }
        Err(e) => {
            // Log failed request
            let _ = state.operational_logger.log_request(&OperationalLogRecord {
                request_id: request.request_id.to_string(),
                caller_id: caller.caller_id,
                route_key: request.route_key.clone(),
                route_key_used: None,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                cost_usd: None,
                latency_ms: Some(latency_ms),
                outcome: "provider_error".into(),
                error_code: Some("502".into()),
                error_detail: Some(e.to_string()),
                fallback_used: false,
                fallback_attempt: 0,
                inbound_hash: Some(sanitized.inbound_hash),
                timestamp: Utc::now().to_rfc3339(),
                tags: None,
            });

            Err(GatewayApiError::BadGateway(e.to_string()))
        }
    }
}

// ---------------------------------------------------------------------------
// Admin handlers
// ---------------------------------------------------------------------------

async fn admin_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let routes = state.route_store.list_routes().unwrap_or_default();
    let active_routes = routes.iter().filter(|r| r.active).count();

    let health_map = state.health_map.read().unwrap();
    let healthy_count = health_map.values().filter(|s| **s == crate::types::HealthStatus::Healthy).count();

    let body = serde_json::json!({
        "status": state.kill_switch.state(),
        "version": env!("CARGO_PKG_VERSION"),
        "routes": {
            "total": routes.len(),
            "active": active_routes,
            "healthy": healthy_count,
        },
        "budgets": {
            "global_daily_usd": state.config.budgets.global_daily_usd,
            "global_spend_24h": state.cost_accountant.global_spend_24h().unwrap_or(0.0),
        },
        "timestamp": Utc::now().to_rfc3339(),
    });
    (StatusCode::OK, Json(body))
}

async fn admin_kill_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mode = body["mode"].as_str().unwrap_or("drain");
    match mode {
        "drain" => {
            state.kill_switch.trigger_drain();
            let _ = state.audit_logger.log_event(&AuditEvent {
                event_type: "kill_switch".into(),
                request_id: None,
                caller_id: None,
                route_key: None,
                severity: "CRITICAL".into(),
                detail: "kill switch triggered: DRAIN".into(),
                timestamp: Utc::now().to_rfc3339(),
            });
            (StatusCode::OK, Json(serde_json::json!({"state": "drain"})))
        }
        "halt" => {
            state.kill_switch.trigger_halt();
            let _ = state.audit_logger.log_event(&AuditEvent {
                event_type: "kill_switch".into(),
                request_id: None,
                caller_id: None,
                route_key: None,
                severity: "CRITICAL".into(),
                detail: "kill switch triggered: HALT".into(),
                timestamp: Utc::now().to_rfc3339(),
            });
            (StatusCode::OK, Json(serde_json::json!({"state": "halted"})))
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "mode must be 'drain' or 'halt'"})),
        ),
    }
}

async fn admin_resume_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let was_stopped = state.kill_switch.resume();
    let _ = state.audit_logger.log_event(&AuditEvent {
        event_type: "kill_switch".into(),
        request_id: None,
        caller_id: None,
        route_key: None,
        severity: "INFO".into(),
        detail: format!("resume requested, was_stopped={was_stopped}"),
        timestamp: Utc::now().to_rfc3339(),
    });
    Json(serde_json::json!({"state": "operational", "was_stopped": was_stopped}))
}

async fn admin_probe_handler(
    axum::extract::Path(_route_key): axum::extract::Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({"error": "manual probe not yet implemented"})),
    )
}

async fn admin_config_reload_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({"error": "config reload not yet implemented"})),
    )
}

async fn admin_cost_summary_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let global = state.cost_accountant.global_spend_24h().unwrap_or(0.0);
    Json(serde_json::json!({
        "window": "24h",
        "global_spend_usd": global,
        "global_ceiling_usd": state.config.budgets.global_daily_usd,
    }))
}

async fn admin_budget_get_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "global_daily_usd": state.config.budgets.global_daily_usd,
        "per_caller_daily_usd": state.config.budgets.per_caller_daily_usd,
        "per_route_daily_usd": state.config.budgets.per_route_daily_usd,
    }))
}
