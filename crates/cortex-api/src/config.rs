use cortex_core::ServiceManifest;

/// Cortex runtime configuration — loaded from environment variables.
#[derive(Clone)]
pub struct CortexConfig {
    pub port: u16,
    pub manifest_path: String,
    pub manifest: ServiceManifest,

    // Cloak connection
    pub cloak_url: String,
    pub cloak_manifest_token: String,

    // Health check settings
    pub health_poll_interval_secs: u64,
    pub health_fail_threshold: u32,
}

impl CortexConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let port = std::env::var("CORTEX_PORT")
            .unwrap_or_else(|_| "9000".into())
            .parse()
            .unwrap_or(9000);

        let manifest_path = std::env::var("CORTEX_MANIFEST")
            .unwrap_or_else(|_| "cortex-manifest.toml".into());

        let manifest = ServiceManifest::from_file(&manifest_path)
            .map_err(|e| format!("Failed to load manifest from {manifest_path}: {e}"))?;

        let cloak_url = std::env::var("CLOAK_URL")
            .unwrap_or_else(|_| "http://localhost:8300".into());

        let cloak_manifest_token = std::env::var("CLOAK_MANIFEST_TOKEN")
            .unwrap_or_default();

        let health_poll_interval_secs = std::env::var("CORTEX_HEALTH_INTERVAL")
            .unwrap_or_else(|_| "10".into())
            .parse()
            .unwrap_or(10);

        let health_fail_threshold = std::env::var("CORTEX_HEALTH_FAIL_THRESHOLD")
            .unwrap_or_else(|_| "2".into())
            .parse()
            .unwrap_or(2);

        Ok(Self {
            port,
            manifest_path,
            manifest,
            cloak_url,
            cloak_manifest_token,
            health_poll_interval_secs,
            health_fail_threshold,
        })
    }
}
