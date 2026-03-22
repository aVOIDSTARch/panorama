pub mod config;
pub mod dispatch;
pub mod identity;
pub mod inbound;
pub mod pipeline;
pub mod sanitization;
pub mod totp;

/// Shared application state for analog-communications.
#[derive(Clone)]
pub struct AppState {
    pub config: config::AnalogConfig,
    pub cloak: cloak_sdk::CloakState,
    pub http: reqwest::Client,
    /// Recognized sender phone numbers (loaded from Datastore at startup).
    pub recognized_senders: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}
