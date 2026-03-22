use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use cloak_core::{CloakConfig, CloakError};
use cloak_permissions::model::PermissionStore;
use cloak_registry::store::ServiceStore;
use cloak_secrets::cache::SecretCache;
use cloak_secrets::infisical::InfisicalClient;

#[derive(Clone)]
pub struct AppState {
    pub config: CloakConfig,
    pub registry: ServiceStore,
    pub permissions: PermissionStore,
    pub secret_cache: SecretCache,
    pub infisical: InfisicalClient,
    pub signing_key: Arc<RwLock<Vec<u8>>>,
    pub halted: Arc<AtomicBool>,
    pub halt_reason: Arc<RwLock<Option<String>>>,
    pub start_time: Instant,
}

impl AppState {
    pub async fn init() -> Result<Self, CloakError> {
        let config = CloakConfig::from_env()?;

        let infisical = InfisicalClient::new(
            config.infisical_url.clone(),
            config.infisical_token.clone(),
            config.infisical_project.clone(),
            config.infisical_env.clone(),
        );

        // Verify Infisical connectivity at startup
        tracing::info!("Checking Infisical connectivity...");
        match infisical.health_check().await {
            Ok(true) => tracing::info!("Infisical is reachable"),
            Ok(false) => tracing::warn!("Infisical health check returned non-200"),
            Err(e) => {
                tracing::warn!("Infisical not reachable at startup: {e} (continuing anyway)");
            }
        }

        let secret_cache = SecretCache::new(infisical.clone(), config.secret_cache_ttl_secs);

        // Initial secret load (non-fatal if Infisical is not yet available)
        if let Err(e) = secret_cache.initial_load().await {
            tracing::warn!("Initial secret load failed: {e} (cache will retry on refresh)");
        }

        // Generate the master signing key for token operations
        let signing_key = cloak_tokens::signing::generate_signing_key();

        Ok(Self {
            config,
            registry: ServiceStore::new(),
            permissions: PermissionStore::new(),
            secret_cache,
            infisical,
            signing_key: Arc::new(RwLock::new(signing_key)),
            halted: Arc::new(AtomicBool::new(false)),
            halt_reason: Arc::new(RwLock::new(None)),
            start_time: Instant::now(),
        })
    }
}
