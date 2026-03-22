use axum::{middleware, Router};
use tower_http::trace::TraceLayer;

use crate::health::health_handler;
use crate::proxy::proxy_request;
use crate::state::AppState;

/// Build the Cortex router.
///
/// Routes:
///   GET  /health                      — aggregated health (no auth)
///   *    /{service_name}/{rest:path}   — proxy to downstream service (auth required)
pub fn build(state: AppState) -> Router {
    let proxy_routes = Router::new()
        .route("/{service_name}/{*rest}", axum::routing::any(proxy_request))
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::cloak_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.cloak.clone(),
            cloak_sdk::middleware::halt_guard,
        ))
        .with_state(state.clone());

    let health_routes = Router::new()
        .route("/health", axum::routing::get(health_handler))
        .with_state(state.clone());

    Router::new()
        .merge(health_routes)
        .merge(proxy_routes)
        .layer(TraceLayer::new_for_http())
}
