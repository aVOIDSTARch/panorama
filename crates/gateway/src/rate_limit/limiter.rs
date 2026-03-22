use crate::config::RateLimitsConfig;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter as GovRateLimiter};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Mutex;

type Limiter = GovRateLimiter<NotKeyed, InMemoryState, DefaultClock>;

pub struct RateLimiter {
    limiters: Mutex<HashMap<String, Limiter>>,
    config: RateLimitsConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitsConfig) -> Self {
        Self {
            limiters: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Check rate limit for caller×route. Returns Ok or Err with retry-after seconds.
    pub fn check(&self, caller_id: &str, route_key: &str) -> Result<(), u64> {
        let key = format!("{caller_id}:{route_key}");
        let mut limiters = self.limiters.lock().unwrap();

        let limiter = limiters.entry(key).or_insert_with(|| {
            let rpm = self.effective_rpm(caller_id, route_key);
            let quota = Quota::per_minute(NonZeroU32::new(rpm).unwrap_or(NonZeroU32::new(1).unwrap()));
            GovRateLimiter::direct(quota)
        });

        match limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => {
                // Estimate retry-after based on the rate
                let rpm = self.effective_rpm(caller_id, route_key);
                let retry_after = if rpm > 0 { 60 / rpm as u64 } else { 60 };
                Err(retry_after.max(1))
            }
        }
    }

    fn effective_rpm(&self, caller_id: &str, route_key: &str) -> u32 {
        // Check caller-specific override
        if let Some(override_config) = self.config.callers.get(caller_id) {
            if let Some(rpm) = override_config.requests_per_minute {
                return rpm;
            }
        }
        // Check route-specific override
        if let Some(override_config) = self.config.routes.get(route_key) {
            if let Some(rpm) = override_config.requests_per_minute {
                return rpm;
            }
        }
        // Default
        self.config.defaults.requests_per_minute
    }
}
