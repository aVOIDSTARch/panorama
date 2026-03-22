use chrono::{Duration, Utc};
use gateway::db;
use gateway::identity::tokens::TokenStore;

fn make_token_store() -> TokenStore {
    let conn = db::init_route_store_in_memory().unwrap();
    TokenStore::new(conn)
}

#[test]
fn issue_and_validate() {
    let store = make_token_store();
    let token = store
        .issue("caller-1", &["*".to_string()], None)
        .unwrap();
    assert!(!token.is_empty());

    let identity = store.validate(&token).unwrap();
    assert_eq!(identity.caller_id, "caller-1");
    assert!(identity.active);
    assert_eq!(identity.allowed_routes, vec!["*"]);
}

#[test]
fn invalid_token_rejected() {
    let store = make_token_store();
    let result = store.validate("nonexistent-token");
    assert!(result.is_err());
}

#[test]
fn revoked_token_rejected() {
    let store = make_token_store();
    let token = store.issue("caller-1", &["*".into()], None).unwrap();
    store.revoke("caller-1").unwrap();
    let result = store.validate(&token);
    assert!(result.is_err());
}

#[test]
fn expired_token_rejected() {
    let store = make_token_store();
    let past = Utc::now() - Duration::hours(1);
    let token = store
        .issue("caller-1", &["*".into()], Some(past))
        .unwrap();
    let result = store.validate(&token);
    assert!(result.is_err());
}

#[test]
fn future_expiry_passes() {
    let store = make_token_store();
    let future = Utc::now() + Duration::hours(24);
    let token = store
        .issue("caller-1", &["*".into()], Some(future))
        .unwrap();
    let identity = store.validate(&token).unwrap();
    assert_eq!(identity.caller_id, "caller-1");
}

#[test]
fn route_access_wildcard() {
    let store = make_token_store();
    let token = store
        .issue("caller-1", &["*".into()], None)
        .unwrap();
    let identity = store.validate(&token).unwrap();
    assert!(TokenStore::check_route_access(&identity, "any-route"));
    assert!(TokenStore::check_route_access(&identity, "another-route"));
}

#[test]
fn route_access_specific() {
    let store = make_token_store();
    let token = store
        .issue("caller-1", &["route-a".into(), "route-b".into()], None)
        .unwrap();
    let identity = store.validate(&token).unwrap();
    assert!(TokenStore::check_route_access(&identity, "route-a"));
    assert!(TokenStore::check_route_access(&identity, "route-b"));
    assert!(!TokenStore::check_route_access(&identity, "route-c"));
}

#[test]
fn list_tokens() {
    let store = make_token_store();
    store.issue("alice", &["*".into()], None).unwrap();
    store.issue("bob", &["route-1".into()], None).unwrap();

    let list = store.list().unwrap();
    assert_eq!(list.len(), 2);
    let caller_ids: Vec<&str> = list.iter().map(|t| t.caller_id.as_str()).collect();
    assert!(caller_ids.contains(&"alice"));
    assert!(caller_ids.contains(&"bob"));
}

#[test]
fn reissue_replaces_previous() {
    let store = make_token_store();
    let token1 = store.issue("caller-1", &["*".into()], None).unwrap();
    let token2 = store.issue("caller-1", &["route-a".into()], None).unwrap();

    // Old token should be invalid (INSERT OR REPLACE)
    assert!(store.validate(&token1).is_err());
    // New token should work
    let identity = store.validate(&token2).unwrap();
    assert_eq!(identity.allowed_routes, vec!["route-a"]);
}
