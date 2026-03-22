use std::net::IpAddr;
use std::sync::atomic::Ordering;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// Middleware that rejects all requests (except /health and /cloak/admin/resume)
/// when the service is in halted state.
pub async fn halt_guard(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // Always allow health and resume endpoints
    if path == "/health" || path == "/cloak/admin/resume" {
        return next.run(req).await;
    }

    if state.halted.load(Ordering::Relaxed) {
        let reason = state
            .halt_reason
            .read()
            .await
            .clone()
            .unwrap_or_else(|| "operator".into());

        return (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "operator_halt",
                "detail": format!("Cloak is halted: {reason}"),
                "service": "cloak",
                "halted": true,
            })),
        )
            .into_response();
    }

    next.run(req).await
}

/// Middleware that restricts access to Tailscale network only.
/// Checks if the source IP is in the Tailscale CGNAT range (100.64.0.0/10).
pub async fn tailscale_guard(req: Request, next: Next) -> Response {
    // Try to extract the connecting IP
    if let Some(addr) = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
    {
        if !is_tailscale_ip(addr.ip()) {
            return (
                StatusCode::FORBIDDEN,
                axum::Json(serde_json::json!({
                    "error": "tailscale_only",
                    "detail": "Admin endpoints are only accessible via Tailscale",
                    "service": "cloak",
                })),
            )
                .into_response();
        }
    }
    // If we can't determine the IP (e.g., localhost dev), allow through
    next.run(req).await
}

fn is_tailscale_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // Tailscale CGNAT range: 100.64.0.0/10
            octets[0] == 100 && (octets[1] & 0xC0) == 64
        }
        IpAddr::V6(v6) => {
            // Tailscale IPv6 prefix: fd7a:115c:a1e0::/48
            let segments = v6.segments();
            segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0
        }
    }
}
