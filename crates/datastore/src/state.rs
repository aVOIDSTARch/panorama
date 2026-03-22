use std::sync::Arc;

use cloak_sdk::{CloakClient, CloakState};

use crate::blob::BlobStore;
use crate::config::DatastoreConfig;
use crate::sqlite::SqliteStore;

/// Shared application state for the Datastore service.
#[derive(Clone)]
pub struct AppState {
    pub config: DatastoreConfig,
    pub cloak: CloakState,
    pub db: Arc<SqliteStore>,
    pub blobs: Arc<BlobStore>,
}

impl AppState {
    pub async fn init(config: DatastoreConfig) -> anyhow::Result<Self> {
        let db = SqliteStore::open(&config.db_path)?;
        let blobs = BlobStore::new(&config.blob_root)?;

        let cloak = CloakState::new();

        // Register with Cloak (non-fatal if Cloak not running)
        let cloak_config = cloak_sdk::CloakConfig::new(
            &config.cloak_url,
            &config.cloak_manifest_token,
            "datastore",
            "storage",
            env!("CARGO_PKG_VERSION"),
        )
        .with_capabilities(vec![
            "objects".into(),
            "queries".into(),
            "blobs".into(),
            "schema".into(),
        ]);

        let client = CloakClient::new(cloak_config);
        match client.register(&cloak).await {
            Ok(halt_url) => {
                tracing::info!("Registered with Cloak");
                client.spawn_halt_listener(cloak.clone(), halt_url);
            }
            Err(e) => {
                tracing::warn!("Cloak registration failed (continuing without): {e}");
            }
        }

        Ok(Self {
            config,
            cloak,
            db: Arc::new(db),
            blobs: Arc::new(blobs),
        })
    }
}
