/// Wheelhouse runtime configuration.
#[derive(Clone)]
pub struct WheelhouseConfig {
    pub port: u16,
    pub cloak_url: String,
    pub cloak_manifest_token: String,
    pub cortex_url: String,
    pub max_agents: usize,
    pub default_model_id: String,
}

impl WheelhouseConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            port: std::env::var("WHEELHOUSE_PORT")
                .unwrap_or_else(|_| "8200".into())
                .parse()
                .unwrap_or(8200),
            cloak_url: std::env::var("CLOAK_URL")
                .unwrap_or_else(|_| "http://localhost:8300".into()),
            cloak_manifest_token: std::env::var("CLOAK_MANIFEST_TOKEN").unwrap_or_default(),
            cortex_url: std::env::var("CORTEX_URL")
                .unwrap_or_else(|_| "http://localhost:9000".into()),
            max_agents: std::env::var("WHEELHOUSE_MAX_AGENTS")
                .unwrap_or_else(|_| "20".into())
                .parse()
                .unwrap_or(20),
            default_model_id: std::env::var("WHEELHOUSE_DEFAULT_MODEL")
                .unwrap_or_else(|_| "ANT-CLD-SONN-035B".into()),
        })
    }
}
