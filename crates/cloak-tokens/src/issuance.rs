use chrono::{Duration, Utc};

use cloak_core::{CloakError, TokenClaims, TokenIssueRequest, TokenIssueResponse};

/// Issue a new token. Signs claims with the provided key.
///
/// In full production flow, this would also register the token with Infisical.
/// For now, we sign locally and return the token.
pub fn issue(req: &TokenIssueRequest, signing_key: &[u8]) -> Result<TokenIssueResponse, CloakError> {
    let now = Utc::now();
    let expires_at = now + Duration::seconds(req.ttl_seconds as i64);

    let claims = TokenClaims {
        job_id: req.job_id.clone(),
        agent_class: req.agent_class.clone(),
        issued_at: now,
        expires_at,
        services: req.services.clone(),
    };

    let token = crate::signing::sign_claims(&claims, signing_key)?;

    Ok(TokenIssueResponse {
        token,
        scope: claims,
    })
}
