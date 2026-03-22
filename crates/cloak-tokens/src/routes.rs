use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use cloak_core::{
    CloakError, TokenIssueRequest, TokenIssueResponse, ValidationRequest, ValidationResponse,
};

use crate::validation;

/// Shared state needed by token routes.
#[derive(Clone)]
pub struct TokenRouterState {
    pub infisical: cloak_secrets::infisical::InfisicalClient,
    pub permissions: cloak_permissions::model::PermissionStore,
    pub signing_key: std::sync::Arc<tokio::sync::RwLock<Vec<u8>>>,
}

pub fn routes() -> Router<TokenRouterState> {
    Router::new()
        .route("/cloak/validate", post(validate_token))
        .route("/cloak/tokens/issue", post(issue_token))
}

async fn validate_token(
    State(state): State<TokenRouterState>,
    Json(req): Json<ValidationRequest>,
) -> Result<Json<ValidationResponse>, CloakError> {
    let key = state.signing_key.read().await;
    let resp = validation::validate(&req, &state.infisical, &state.permissions, &key).await?;
    Ok(Json(resp))
}

async fn issue_token(
    State(state): State<TokenRouterState>,
    Json(req): Json<TokenIssueRequest>,
) -> Result<Json<TokenIssueResponse>, CloakError> {
    let key = state.signing_key.read().await;
    let resp = crate::issuance::issue(&req, &key)?;
    Ok(Json(resp))
}
