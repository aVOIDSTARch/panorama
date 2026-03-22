use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::infisical::InfisicalClient;
use cloak_core::CloakError;

#[derive(Debug, Clone)]
struct CachedSecret {
    value: String,
    version: u64,
    fetched_at: Instant,
}

/// TTL-based cache for secrets. Token validation NEVER uses this cache.
#[derive(Debug, Clone)]
pub struct SecretCache {
    inner: Arc<RwLock<HashMap<String, CachedSecret>>>,
    ttl: Duration,
    client: InfisicalClient,
}

impl SecretCache {
    pub fn new(client: InfisicalClient, ttl_secs: u64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
            client,
        }
    }

    /// Initial load of all secrets from Infisical.
    pub async fn initial_load(&self) -> Result<(), CloakError> {
        let secrets = self.client.fetch_secrets().await?;
        let mut cache = self.inner.write().await;
        let now = Instant::now();
        for s in secrets {
            cache.insert(
                s.key,
                CachedSecret {
                    value: s.value,
                    version: s.version,
                    fetched_at: now,
                },
            );
        }
        info!("Secret cache loaded with {} entries", cache.len());
        Ok(())
    }

    /// Get a secret, refreshing from Infisical if expired.
    pub async fn get(&self, key: &str) -> Result<String, CloakError> {
        // Check cache first
        {
            let cache = self.inner.read().await;
            if let Some(entry) = cache.get(key) {
                if entry.fetched_at.elapsed() < self.ttl {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Cache miss or expired — fetch from Infisical
        let secret = self.client.fetch_secret(key).await?;
        let value = secret.value.clone();

        let mut cache = self.inner.write().await;
        cache.insert(
            key.to_string(),
            CachedSecret {
                value: secret.value,
                version: secret.version,
                fetched_at: Instant::now(),
            },
        );

        Ok(value)
    }

    /// Background refresh task: re-pulls all secrets on a timer.
    pub async fn background_refresh(self) {
        loop {
            tokio::time::sleep(self.ttl).await;
            match self.client.fetch_secrets().await {
                Ok(secrets) => {
                    let mut cache = self.inner.write().await;
                    let now = Instant::now();
                    for s in secrets {
                        cache.insert(
                            s.key,
                            CachedSecret {
                                value: s.value,
                                version: s.version,
                                fetched_at: now,
                            },
                        );
                    }
                    info!("Secret cache refreshed ({} entries)", cache.len());
                }
                Err(e) => {
                    warn!("Secret cache refresh failed: {e}");
                }
            }
        }
    }

    /// Get the underlying Infisical client for direct (uncached) operations.
    pub fn infisical(&self) -> &InfisicalClient {
        &self.client
    }
}
