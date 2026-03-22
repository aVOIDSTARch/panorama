pub mod session;
pub mod webauthn;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Simple session-based auth middleware.
///
/// In the future this will be upgraded to YubiKey FIDO2/WebAuthn.
/// For now, uses a shared secret from environment variable.
pub async fn require_auth(request: Request, next: Next) -> Response {
    // Allow static assets and health through
    let path = request.uri().path();
    if path.starts_with("/static/") || path == "/health" || path == "/login" || path.starts_with("/auth/") {
        return next.run(request).await;
    }

    // Check session cookie
    let has_session = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|c| c.contains("admin_session="))
        .unwrap_or(false);

    if !has_session {
        // Redirect to login
        return (
            StatusCode::TEMPORARY_REDIRECT,
            [("location", "/login")],
            "",
        )
            .into_response();
    }

    next.run(request).await
}
