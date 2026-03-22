//! cloak-sdk — Reusable Cloak client library for Panorama services.
//!
//! Any Rust service in Panorama can depend on this crate to:
//! 1. Register with Cloak at startup
//! 2. Listen for SSE halt/key-rotation events
//! 3. Verify bearer tokens locally via HMAC-SHA256
//! 4. Use Axum middleware for auth + halt guards

pub mod client;
pub mod middleware;
pub mod sse;
pub mod state;

pub use client::{CloakClient, CloakConfig};
pub use state::CloakState;
