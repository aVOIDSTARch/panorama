pub mod config;
pub mod dispatch;
pub mod identity;
pub mod inbound;
pub mod pipeline;
pub mod sanitization;

/// Shared application state for analog-communications.
#[derive(Clone)]
pub struct AppState {
    pub config: config::AnalogConfig,
    pub cloak: cloak_sdk::CloakState,
    pub http: reqwest::Client,
}
