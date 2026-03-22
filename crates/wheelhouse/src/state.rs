use std::sync::Arc;

use cloak_sdk::{CloakClient, CloakState};
use wheelhouse_agent_lifecycle::AgentPool;

use crate::config::WheelhouseConfig;
use crate::orchestrator::Orchestrator;

/// Shared application state for Wheelhouse.
#[derive(Clone)]
pub struct AppState {
    pub config: WheelhouseConfig,
    pub cloak: CloakState,
    pub orchestrator: Arc<Orchestrator>,
}

impl AppState {
    pub async fn init(config: WheelhouseConfig) -> anyhow::Result<Self> {
        let cloak = CloakState::new();

        // Register with Cloak
        let cloak_config = cloak_sdk::CloakConfig::new(
            &config.cloak_url,
            &config.cloak_manifest_token,
            "wheelhouse",
            "orchestration",
            env!("CARGO_PKG_VERSION"),
        )
        .with_capabilities(vec![
            "job_dispatch".into(),
            "agent_management".into(),
            "task_lifecycle".into(),
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

        let agent_pool = Arc::new(AgentPool::new(config.max_agents));
        let orchestrator = Arc::new(Orchestrator::new(
            &config.default_model_id,
            agent_pool,
        ));

        Ok(Self {
            config,
            cloak,
            orchestrator,
        })
    }
}
