use chrono::{Duration, Utc};
use cloak_core::{OperationClass, ServiceScope, TokenClaims};
use cloak_tokens::signing::sign_claims;

/// Sign a test token valid for 300s, granting Read access to the given service.
pub fn sign_test_token(signing_key: &[u8], service_id: &str) -> String {
    let claims = TokenClaims {
        job_id: "test-job".into(),
        agent_class: "test-agent".into(),
        issued_at: Utc::now(),
        expires_at: Utc::now() + Duration::seconds(300),
        services: vec![ServiceScope {
            service: service_id.into(),
            operation_class: OperationClass::Read,
            resources: vec!["*".into()],
        }],
    };
    sign_claims(&claims, signing_key).expect("Failed to sign test token")
}

/// Sign a token that is already expired.
pub fn sign_expired_token(signing_key: &[u8], service_id: &str) -> String {
    let claims = TokenClaims {
        job_id: "expired-job".into(),
        agent_class: "test-agent".into(),
        issued_at: Utc::now() - Duration::seconds(600),
        expires_at: Utc::now() - Duration::seconds(300),
        services: vec![ServiceScope {
            service: service_id.into(),
            operation_class: OperationClass::Read,
            resources: vec!["*".into()],
        }],
    };
    sign_claims(&claims, signing_key).expect("Failed to sign expired token")
}
