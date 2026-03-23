use axum::{middleware, routing, Router};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use admin_interface::api;
use admin_interface::auth;

#[tokio::main]
async fn main() {
    panorama_logging::init("admin-interface", Some("data/panorama_logs.db"));

    let state = Arc::new(api::health::AppState::from_env());
    let port: u16 = std::env::var("ADMIN_PORT")
        .unwrap_or_else(|_| "8400".into())
        .parse()
        .unwrap_or(8400);

    let app = Router::new()
        // Pages
        .route("/", routing::get(api::dashboard))
        .route("/logs", routing::get(api::logs_page))
        .route("/errors", routing::get(api::errors_page))
        .route("/login", routing::get(auth::session::login_page))
        .route("/login", routing::post(auth::session::login_submit))
        .route("/auth/register", routing::get(auth::webauthn::register_page))
        .route("/auth/webauthn/register/start", routing::post(auth::webauthn::register_start))
        .route("/auth/webauthn/register/finish", routing::post(auth::webauthn::register_finish))
        .route("/auth/webauthn/auth/start", routing::post(auth::webauthn::auth_start))
        .route("/auth/webauthn/auth/finish", routing::post(auth::webauthn::auth_finish))
        // HTMX API fragments — read-only
        .route("/api/health", routing::get(api::health::health_panel))
        .route("/api/services", routing::get(api::health::services_panel))
        .route("/api/logs", routing::get(api::logs::logs_panel))
        .route("/api/errors/summary", routing::get(api::errors::errors_summary_panel))
        .route("/api/errors/recent", routing::get(api::errors::errors_recent_panel))
        .route("/api/halt", routing::get(api::halt::halt_panel))
        .route("/api/permissions", routing::get(api::permissions::permissions_panel))
        .route("/api/wheelhouse", routing::get(api::wheelhouse::wheelhouse_panel))
        .route("/api/config", routing::get(api::config_viewer::config_panel))
        .route("/api/identity", routing::get(api::identity::identity_panel))
        // HTMX API fragments — mutations
        .route("/api/halt/all", routing::post(api::halt::halt_all))
        .route("/api/halt/resume", routing::post(api::halt::resume))
        .route("/api/halt/service/:service_id", routing::post(api::halt::halt_service))
        .route("/api/permissions/add", routing::post(api::permissions::add_permission))
        .route("/api/permissions/remove", routing::delete(api::permissions::remove_permission))
        // Auth middleware (skip for /login, /health, /static)
        .layer(middleware::from_fn(auth::require_auth))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let bind_addr = resolve_bind_address(port);
    tracing::info!("Admin interface listening on {bind_addr}");

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

/// Resolve the bind address for the admin interface.
///
/// If `TAILSCALE_INTERFACE` is set (e.g. "tailscale0"), resolves its IPv4
/// address and binds only to that. This restricts the admin panel to the
/// Tailscale network. Falls back to `127.0.0.1` if the interface can't be
/// resolved (fail-closed). If the env var is unset, binds to `0.0.0.0`.
fn resolve_bind_address(port: u16) -> String {
    let interface = match std::env::var("TAILSCALE_INTERFACE") {
        Ok(iface) if !iface.is_empty() => iface,
        _ => return format!("0.0.0.0:{port}"),
    };

    // Query the interface for its IPv4 address (platform-specific command)
    let output = if cfg!(target_os = "macos") {
        std::process::Command::new("ifconfig")
            .arg(&interface)
            .output()
    } else {
        std::process::Command::new("ip")
            .args(["addr", "show", &interface])
            .output()
    };

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Both `ip addr` and `ifconfig` output "inet X.X.X.X" lines
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("inet ") && !trimmed.starts_with("inet6") {
                    if let Some(addr) = trimmed
                        .strip_prefix("inet ")
                        .and_then(|s| s.split(|c: char| c == '/' || c.is_whitespace()).next())
                    {
                        tracing::info!(
                            "Binding admin interface to {interface} ({addr})"
                        );
                        return format!("{addr}:{port}");
                    }
                }
            }
            tracing::warn!(
                "Could not find IPv4 address on {interface} — falling back to 127.0.0.1"
            );
            format!("127.0.0.1:{port}")
        }
        Ok(_) => {
            tracing::warn!(
                "Interface {interface} not found — falling back to 127.0.0.1"
            );
            format!("127.0.0.1:{port}")
        }
        Err(e) => {
            tracing::warn!(
                "Failed to query interface {interface}: {e} — falling back to 127.0.0.1"
            );
            format!("127.0.0.1:{port}")
        }
    }
}
