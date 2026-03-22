use cloak_core::TokenClaims;

use crate::types::CallerIdentity;

/// Convert Cloak TokenClaims into the Gateway's CallerIdentity type.
///
/// Mapping:
///   - job_id   -> caller_id
///   - services  -> allowed_routes (each service name becomes a route wildcard)
///   - issued_at / expires_at carried through
pub fn claims_to_identity(claims: &TokenClaims) -> CallerIdentity {
    CallerIdentity {
        caller_id: claims.job_id.clone(),
        token_hash: String::new(), // Cloak uses HMAC verification, no stored hash
        issued_at: claims.issued_at,
        expires_at: Some(claims.expires_at),
        allowed_routes: claims
            .services
            .iter()
            .map(|s| s.service.clone())
            .collect(),
        active: true,
    }
}

/// Check if a caller (from Cloak claims) is allowed to use a specific route.
///
/// Wildcards: if services list contains "*" or "gateway", all routes are allowed.
/// Otherwise, checks if the route_key appears in the service scope list.
pub fn check_route_access(identity: &CallerIdentity, route_key: &str) -> bool {
    if identity.allowed_routes.is_empty() {
        return true;
    }
    identity
        .allowed_routes
        .iter()
        .any(|r| r == "*" || r == "gateway" || r == route_key)
}
