use gateway::config::{RateLimitDefaults, RateLimitOverride, RateLimitsConfig};
use gateway::rate_limit::limiter::RateLimiter;
use std::collections::HashMap;

fn default_config(rpm: u32) -> RateLimitsConfig {
    RateLimitsConfig {
        defaults: RateLimitDefaults {
            requests_per_minute: rpm,
            requests_per_hour: 500,
        },
        callers: HashMap::new(),
        routes: HashMap::new(),
    }
}

#[test]
fn within_rate_limit_passes() {
    let limiter = RateLimiter::new(default_config(60));
    // First request should always pass
    assert!(limiter.check("caller-1", "route-1").is_ok());
}

#[test]
fn exceeding_rate_limit_returns_retry_after() {
    // Set limit to 1 RPM — only 1 request allowed per minute
    let limiter = RateLimiter::new(default_config(1));
    // First request consumes the token
    assert!(limiter.check("caller-1", "route-1").is_ok());
    // Second should be rejected
    let err = limiter.check("caller-1", "route-1").unwrap_err();
    assert!(err >= 1, "retry_after should be at least 1 second");
}

#[test]
fn different_caller_route_pairs_independent() {
    let limiter = RateLimiter::new(default_config(1));
    assert!(limiter.check("caller-1", "route-1").is_ok());
    // Different caller×route pair should have its own bucket
    assert!(limiter.check("caller-2", "route-1").is_ok());
    assert!(limiter.check("caller-1", "route-2").is_ok());
}

#[test]
fn caller_override_applies() {
    let mut config = default_config(60);
    config.callers.insert(
        "vip-caller".to_string(),
        RateLimitOverride {
            requests_per_minute: Some(1),
            requests_per_hour: None,
        },
    );
    let limiter = RateLimiter::new(config);

    // VIP caller gets 1 RPM override
    assert!(limiter.check("vip-caller", "route-1").is_ok());
    assert!(limiter.check("vip-caller", "route-1").is_err());

    // Regular caller still gets 60 RPM
    assert!(limiter.check("regular-caller", "route-1").is_ok());
    assert!(limiter.check("regular-caller", "route-1").is_ok());
}

#[test]
fn route_override_applies() {
    let mut config = default_config(60);
    config.routes.insert(
        "expensive-route".to_string(),
        RateLimitOverride {
            requests_per_minute: Some(1),
            requests_per_hour: None,
        },
    );
    let limiter = RateLimiter::new(config);

    assert!(limiter.check("caller-1", "expensive-route").is_ok());
    assert!(limiter.check("caller-1", "expensive-route").is_err());
}
