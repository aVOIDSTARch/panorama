//! Cortex auth — thin wrapper around cloak-sdk for Cortex-specific auth patterns.

pub use cloak_sdk::{CloakClient, CloakConfig, CloakState};
pub use cloak_sdk::middleware::{cloak_auth, halt_guard};
