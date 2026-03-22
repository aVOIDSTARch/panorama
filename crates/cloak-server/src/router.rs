use axum::Router;
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::{admin, health};

/// Build the unified Axum router combining all subsystem routes.
pub fn build(state: AppState) -> Router {
    let token_state = cloak_tokens::routes::TokenRouterState {
        infisical: state.infisical.clone(),
        permissions: state.permissions.clone(),
        signing_key: state.signing_key.clone(),
    };

    let registry_state = cloak_registry::routes::RegistryRouterState {
        store: state.registry.clone(),
        server_port: state.config.port,
    };

    Router::new()
        // Health (no auth, no halt guard)
        .merge(health::routes(state.clone()))
        // Token routes (validate, issue)
        .merge(
            cloak_tokens::routes::routes()
                .with_state(token_state),
        )
        // Registry routes (register, halt-stream, list)
        .merge(
            cloak_registry::routes::routes()
                .with_state(registry_state),
        )
        // Secrets routes
        .merge(
            cloak_secrets::routes::routes()
                .with_state(state.secret_cache.clone()),
        )
        // Permissions admin routes
        .merge(
            cloak_permissions::routes::routes()
                .with_state(state.permissions.clone()),
        )
        // Admin routes (halt/resume)
        .merge(admin::routes(state.clone()))
        // Global middleware
        .layer(TraceLayer::new_for_http())
}
