use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Deduplicator {
    cache: Mutex<HashMap<String, (String, Instant)>>,
    window: Duration,
}

impl Deduplicator {
    pub fn new(window_secs: u64) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check if a prompt hash has been seen recently.
    /// Returns Ok(()) if unique, Err(original_request_id) if duplicate.
    pub fn check(&self, inbound_hash: &str, request_id: &str) -> Result<(), String> {
        let mut cache = self.cache.lock().unwrap();
        let now = Instant::now();

        // Clean expired entries
        cache.retain(|_, (_, ts)| now.duration_since(*ts) < self.window);

        if let Some((original_id, _)) = cache.get(inbound_hash) {
            return Err(original_id.clone());
        }

        cache.insert(inbound_hash.to_string(), (request_id.to_string(), now));
        Ok(())
    }
}
