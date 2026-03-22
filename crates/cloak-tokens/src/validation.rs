use cloak_core::{CloakError, TokenClaims, ValidationRequest, ValidationResponse};
use cloak_permissions::engine::check_permission;
use cloak_permissions::model::PermissionStore;
use cloak_secrets::infisical::InfisicalClient;

/// Validate a token per-request. This ALWAYS calls Infisical — never cached.
///
/// Flow:
/// 1. Call Infisical to verify token is not revoked
/// 2. Check token scope against permission store
/// 3. Return allowed/denied with reason
pub async fn validate(
    req: &ValidationRequest,
    infisical: &InfisicalClient,
    permissions: &PermissionStore,
    signing_key: &[u8],
) -> Result<ValidationResponse, CloakError> {
    // Step 1: Verify the token signature and decode claims
    let claims = crate::signing::verify_and_decode(&req.token, signing_key)?;

    // Step 2: Check expiration
    let now = chrono::Utc::now();
    if claims.expires_at < now {
        return Ok(ValidationResponse {
            allowed: false,
            reason: "token_expired".into(),
        });
    }

    // Step 3: Validate against Infisical (never cached — revoked token is dead)
    let infisical_result = infisical.validate_token(&req.token).await?;
    if !infisical_result.valid {
        return Ok(ValidationResponse {
            allowed: false,
            reason: "token_revoked".into(),
        });
    }

    // Step 4: Check scope against permission store
    let allowed =
        check_permission(permissions, &claims, &req.service, &req.operation, &req.resource).await;

    if !allowed {
        return Ok(ValidationResponse {
            allowed: false,
            reason: format!(
                "scope_denied: {} {} on {}",
                req.operation, req.resource, req.service
            ),
        });
    }

    Ok(ValidationResponse {
        allowed: true,
        reason: "ok".into(),
    })
}

/// Extract and verify a bearer token from a request, returning the claims.
/// Used by internal route handlers that need token auth.
pub fn extract_and_verify(
    auth_header: Option<&str>,
    signing_key: Option<&[u8]>,
) -> Result<TokenClaims, CloakError> {
    let key = signing_key.ok_or(CloakError::NoSigningKey)?;

    let header = auth_header.ok_or(CloakError::MissingToken)?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or(CloakError::MissingToken)?;

    crate::signing::verify_and_decode(token, key)
}
