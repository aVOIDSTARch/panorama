use cloak_core::{OperationClass, TokenClaims};

use crate::model::PermissionStore;

/// Check whether a token's claims allow a specific operation on a specific
/// service and resource.
///
/// This checks both the token's embedded service scopes AND the permission
/// store's policy entries.
pub async fn check_permission(
    store: &PermissionStore,
    claims: &TokenClaims,
    service: &str,
    operation: &str,
    resource: &str,
) -> bool {
    let required_op = match operation {
        "read" => OperationClass::Read,
        "write" => OperationClass::Write,
        "admin" => OperationClass::Admin,
        _ => return false,
    };

    // First check the token's embedded service scopes
    let scope_match = claims.services.iter().any(|s| {
        s.service == service
            && s.operation_class.satisfies(&required_op)
            && resource_matches(&s.resources, resource)
    });

    if !scope_match {
        return false;
    }

    // Then check the permission store's policy entries
    let entries = store.list().await;

    // If no policy entries exist, token scope alone is sufficient
    if entries.is_empty() {
        return true;
    }

    entries.iter().any(|e| {
        identity_matches(&e.identity_pattern, &claims.agent_class)
            && e.service == service
            && e.operation_class.satisfies(&required_op)
            && resource_matches(&e.resources, resource)
    })
}

fn resource_matches(allowed: &[String], requested: &str) -> bool {
    allowed.iter().any(|r| r == "*" || r == requested)
}

fn identity_matches(pattern: &str, identity: &str) -> bool {
    pattern == "*" || pattern == identity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PermissionEntry;
    use chrono::Utc;
    use cloak_core::ServiceScope;

    fn make_claims(service: &str, op: OperationClass, resources: Vec<&str>) -> TokenClaims {
        TokenClaims {
            job_id: "job-1".into(),
            agent_class: "researcher".into(),
            issued_at: Utc::now(),
            expires_at: Utc::now(),
            services: vec![ServiceScope {
                service: service.into(),
                operation_class: op,
                resources: resources.into_iter().map(String::from).collect(),
            }],
        }
    }

    #[tokio::test]
    async fn test_read_satisfies_read() {
        let store = PermissionStore::new();
        let claims = make_claims("episteme", OperationClass::Read, vec!["*"]);
        assert!(check_permission(&store, &claims, "episteme", "read", "/api/v1/docs").await);
    }

    #[tokio::test]
    async fn test_read_does_not_satisfy_write() {
        let store = PermissionStore::new();
        let claims = make_claims("episteme", OperationClass::Read, vec!["*"]);
        assert!(!check_permission(&store, &claims, "episteme", "write", "/api/v1/docs").await);
    }

    #[tokio::test]
    async fn test_admin_satisfies_all() {
        let store = PermissionStore::new();
        let claims = make_claims("episteme", OperationClass::Admin, vec!["*"]);
        assert!(check_permission(&store, &claims, "episteme", "read", "/any").await);
        assert!(check_permission(&store, &claims, "episteme", "write", "/any").await);
        assert!(check_permission(&store, &claims, "episteme", "admin", "/any").await);
    }

    #[tokio::test]
    async fn test_wrong_service() {
        let store = PermissionStore::new();
        let claims = make_claims("episteme", OperationClass::Admin, vec!["*"]);
        assert!(!check_permission(&store, &claims, "cerebro", "read", "/any").await);
    }

    #[tokio::test]
    async fn test_specific_resource() {
        let store = PermissionStore::new();
        let claims = make_claims("episteme", OperationClass::Read, vec!["/api/v1/docs"]);
        assert!(check_permission(&store, &claims, "episteme", "read", "/api/v1/docs").await);
        assert!(
            !check_permission(&store, &claims, "episteme", "read", "/api/v1/other").await
        );
    }

    #[tokio::test]
    async fn test_policy_enforcement() {
        let store = PermissionStore::with_entries(vec![PermissionEntry {
            identity_pattern: "researcher".into(),
            service: "episteme".into(),
            operation_class: OperationClass::Read,
            resources: vec!["*".into()],
        }]);
        let claims = make_claims("episteme", OperationClass::Write, vec!["*"]);
        // Token has write, but policy only allows read for researchers
        assert!(!check_permission(&store, &claims, "episteme", "write", "/any").await);
        assert!(check_permission(&store, &claims, "episteme", "read", "/any").await);
    }
}
