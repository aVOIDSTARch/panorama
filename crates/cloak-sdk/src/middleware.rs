use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::warn;

use cloak_core::{CloakError, TokenClaims};
use cloak_tokens::signing::verify_and_decode;

use crate::state::CloakState;

/// Axum middleware that rejects all requests when the service is halted.
///
/// Allows `/health` through so monitoring can observe the halted state.
pub async fn halt_guard(
    State(state): State<CloakState>,
    request: Request,
    next: Next,
) -> Response {
    // Always allow health checks through
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    if state.is_halted().await {
        let reason = state
            .halt_reason()
            .await
            .unwrap_or_else(|| "unknown".into());

        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": "service_halted",
                "detail": format!("Service halted: {reason}"),
                "halted": true,
            })),
        )
            .into_response();
    }

    next.run(request).await
}

/// Axum middleware that extracts and verifies the bearer token.
///
/// On success, inserts `TokenClaims` into request extensions.
/// On failure, returns appropriate 401/403/503.
pub async fn cloak_auth(
    State(state): State<CloakState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Allow health checks without auth
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    let key = match state.signing_key().await {
        Some(k) => k,
        None => {
            return CloakError::NoSigningKey.into_response();
        }
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = match auth_header.strip_prefix("Bearer ") {
        Some(t) => t,
        None => {
            return CloakError::MissingToken.into_response();
        }
    };

    let claims = match verify_and_decode(token, &key) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Token verification failed");
            return e.into_response();
        }
    };

    // Check expiration
    let now = chrono::Utc::now();
    if claims.expires_at < now {
        return CloakError::InvalidToken("Token expired".into()).into_response();
    }

    // Insert claims into request extensions for handlers to access
    request.extensions_mut().insert(claims);
    next.run(request).await
}

/// Helper to extract TokenClaims from request extensions in handlers.
///
/// Use after `cloak_auth` middleware is applied.
pub fn extract_claims(request: &Request) -> Option<&TokenClaims> {
    request.extensions().get::<TokenClaims>()
}

// Usage example for consuming services:
//
// ```rust
// use axum::{Router, middleware};
// use cloak_sdk::{CloakState, middleware::{halt_guard, cloak_auth}};
//
// let state = CloakState::new();
// let app = Router::new()
//     .route("/api/data", get(handler))
//     .layer(middleware::from_fn_with_state(state.clone(), cloak_auth))
//     .layer(middleware::from_fn_with_state(state.clone(), halt_guard))
//     .route("/health", get(health_handler));
// ```
