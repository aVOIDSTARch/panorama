use chrono::Utc;
use gateway::db;
use gateway::routes::store::RouteStore;
use gateway::types::{Provider, Route};

fn make_store() -> RouteStore {
    let conn = db::init_route_store_in_memory().unwrap();
    RouteStore::new(conn)
}

fn sample_route(key: &str) -> Route {
    Route {
        route_key: key.into(),
        display_name: format!("Route {key}"),
        provider: Provider::Anthropic,
        model_id: "claude-sonnet-4-20250514".into(),
        endpoint_url: "https://api.anthropic.com".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        max_input_tokens: 200_000,
        max_output_tokens: 8192,
        cost_per_input_token_usd: 0.000003,
        cost_per_output_token_usd: 0.000015,
        fallback_chain: vec!["fallback-1".into()],
        health_probe_interval_secs: 300,
        active: true,
        version: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        tags: vec!["default".into()],
    }
}

#[test]
fn add_and_get_route() {
    let store = make_store();
    store.add_route(&sample_route("claude-sonnet")).unwrap();

    let route = store.get_route("claude-sonnet").unwrap().unwrap();
    assert_eq!(route.route_key, "claude-sonnet");
    assert_eq!(route.display_name, "Route claude-sonnet");
    assert_eq!(route.provider, Provider::Anthropic);
    assert_eq!(route.model_id, "claude-sonnet-4-20250514");
    assert!(route.active);
    assert_eq!(route.version, 1);
}

#[test]
fn get_nonexistent_route_returns_none() {
    let store = make_store();
    let result = store.get_route("doesnt-exist").unwrap();
    assert!(result.is_none());
}

#[test]
fn list_routes_ordered() {
    let store = make_store();
    store.add_route(&sample_route("b-route")).unwrap();
    store.add_route(&sample_route("a-route")).unwrap();

    let routes = store.list_routes().unwrap();
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].route_key, "a-route");
    assert_eq!(routes[1].route_key, "b-route");
}

#[test]
fn update_route_increments_version() {
    let store = make_store();
    store.add_route(&sample_route("test-route")).unwrap();

    store
        .update_route("test-route", "max_output_tokens", "16384")
        .unwrap();

    let route = store.get_route("test-route").unwrap().unwrap();
    assert_eq!(route.version, 2);
}

#[test]
fn update_route_archives_to_history() {
    let store = make_store();
    store.add_route(&sample_route("test-route")).unwrap();

    store
        .update_route("test-route", "display_name", "New Name")
        .unwrap();

    let history = store.route_history("test-route").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].version, 1);
    assert_eq!(history[0].display_name, "Route test-route");
}

#[test]
fn update_disallowed_field_rejected() {
    let store = make_store();
    store.add_route(&sample_route("test-route")).unwrap();

    let result = store.update_route("test-route", "route_key", "new-key");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cannot update field"));
}

#[test]
fn disable_and_enable_route() {
    let store = make_store();
    store.add_route(&sample_route("test-route")).unwrap();

    store.disable_route("test-route").unwrap();
    let route = store.get_route("test-route").unwrap().unwrap();
    assert!(!route.active);
    assert_eq!(route.version, 2);

    store.enable_route("test-route").unwrap();
    let route = store.get_route("test-route").unwrap().unwrap();
    assert!(route.active);
    assert_eq!(route.version, 3);
}

#[test]
fn rollback_restores_previous_version() {
    let store = make_store();
    store.add_route(&sample_route("test-route")).unwrap();

    // Update to v2
    store
        .update_route("test-route", "model_id", "new-model-v2")
        .unwrap();
    let v2 = store.get_route("test-route").unwrap().unwrap();
    assert_eq!(v2.model_id, "new-model-v2");
    assert_eq!(v2.version, 2);

    // Rollback to v1
    store.rollback_route("test-route", 1).unwrap();
    let rolled_back = store.get_route("test-route").unwrap().unwrap();
    assert_eq!(rolled_back.model_id, "claude-sonnet-4-20250514");
    assert_eq!(rolled_back.version, 3); // version increments on rollback
}

#[test]
fn multiple_updates_build_full_history() {
    let store = make_store();
    store.add_route(&sample_route("test-route")).unwrap();
    store.update_route("test-route", "display_name", "V2").unwrap();
    store.update_route("test-route", "display_name", "V3").unwrap();
    store.update_route("test-route", "display_name", "V4").unwrap();

    let history = store.route_history("test-route").unwrap();
    assert_eq!(history.len(), 3); // v1, v2, v3 in history (current is v4)
}

#[test]
fn provider_serialization_roundtrip() {
    let store = make_store();
    let mut route = sample_route("openai-route");
    route.provider = Provider::OpenAI;
    store.add_route(&route).unwrap();

    let fetched = store.get_route("openai-route").unwrap().unwrap();
    assert_eq!(fetched.provider, Provider::OpenAI);
}

#[test]
fn custom_provider_serialization() {
    let store = make_store();
    let mut route = sample_route("custom-route");
    route.provider = Provider::Custom {
        name: "my-provider".into(),
    };
    store.add_route(&route).unwrap();

    let fetched = store.get_route("custom-route").unwrap().unwrap();
    assert_eq!(
        fetched.provider,
        Provider::Custom {
            name: "my-provider".into()
        }
    );
}

#[test]
fn fallback_chain_and_tags_roundtrip() {
    let store = make_store();
    let mut route = sample_route("chain-test");
    route.fallback_chain = vec!["fb-1".into(), "fb-2".into(), "fb-3".into()];
    route.tags = vec!["prod".into(), "fast".into()];
    store.add_route(&route).unwrap();

    let fetched = store.get_route("chain-test").unwrap().unwrap();
    assert_eq!(fetched.fallback_chain, vec!["fb-1", "fb-2", "fb-3"]);
    assert_eq!(fetched.tags, vec!["prod", "fast"]);
}
