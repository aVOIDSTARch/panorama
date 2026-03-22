use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use cloak_sdk::{CloakClient, CloakConfig, CloakState};
use cortex_core::FailureState;
use tracing::{info, warn};

use crate::config::CortexConfig;

/// Per-service runtime state tracked by Cortex.
#[derive(Clone)]
pub struct ServiceState {
    pub failure_state: FailureState,
    pub consecutive_failures: u32,
}

impl ServiceState {
    pub fn new() -> Self {
        Self {
            failure_state: FailureState::Healthy,
            consecutive_failures: 0,
        }
    }
}

/// Shared application state for Cortex.
#[derive(Clone)]
pub struct AppState {
    pub config: CortexConfig,
    pub cloak: CloakState,
    pub cloak_client: Arc<CloakClient>,
    pub http: reqwest::Client,
    pub service_states: Arc<RwLock<HashMap<String, ServiceState>>>,
}

impl AppState {
    pub async fn init(config: CortexConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let cloak_state = CloakState::new();

        let cloak_config = CloakConfig::new(
            &config.cloak_url,
            &config.cloak_manifest_token,
            "cortex",
            "proxy",
            env!("CARGO_PKG_VERSION"),
        )
        .with_capabilities(vec!["proxy".into(), "health".into()]);

        let cloak_client = CloakClient::new(cloak_config);

        // Register with Cloak (non-fatal if Cloak is not running — for dev mode)
        match cloak_client.register(&cloak_state).await {
            Ok(halt_url) => {
                cloak_client.spawn_halt_listener(cloak_state.clone(), halt_url);
                info!("Cortex registered with Cloak");
            }
            Err(e) => {
                warn!(error = %e, "Cloak registration failed — running without auth");
            }
        }

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        // Initialize per-service states
        let mut states = HashMap::new();
        for svc_name in config.manifest.services.keys() {
            states.insert(svc_name.clone(), ServiceState::new());
        }

        Ok(Self {
            config,
            cloak: cloak_state,
            cloak_client: Arc::new(cloak_client),
            http,
            service_states: Arc::new(RwLock::new(states)),
        })
    }

    /// Get the failure state for a service.
    pub async fn service_failure_state(&self, service: &str) -> Option<FailureState> {
        self.service_states
            .read()
            .await
            .get(service)
            .map(|s| s.failure_state.clone())
    }

    /// Record a health check success for a service.
    pub async fn record_health_success(&self, service: &str) {
        let mut states = self.service_states.write().await;
        if let Some(state) = states.get_mut(service) {
            state.failure_state = FailureState::Healthy;
            state.consecutive_failures = 0;
        }
    }

    /// Record a health check failure for a service.
    pub async fn record_health_failure(&self, service: &str) {
        let threshold = self.config.health_fail_threshold;
        let mut states = self.service_states.write().await;
        if let Some(state) = states.get_mut(service) {
            state.consecutive_failures += 1;
            if state.consecutive_failures >= threshold {
                state.failure_state = state.failure_state.escalate();
                warn!(
                    service = service,
                    failures = state.consecutive_failures,
                    state = ?state.failure_state,
                    "Service failure state escalated"
                );
            }
        }
    }
}
