use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use webauthn_rs::prelude::*;

use crate::api::health::AppState;

/// Stored credentials for the single admin operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub passkeys: Vec<Passkey>,
}

impl Default for StoredCredentials {
    fn default() -> Self {
        Self {
            passkeys: Vec::new(),
        }
    }
}

/// Load credentials from disk.
pub fn load_credentials(path: &str) -> StoredCredentials {
    match std::fs::read_to_string(path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => StoredCredentials::default(),
    }
}

/// Save credentials to disk.
fn save_credentials(path: &str, creds: &StoredCredentials) -> Result<(), String> {
    let json = serde_json::to_string_pretty(creds).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Check if any WebAuthn credentials are registered.
pub fn has_credentials(state: &AppState) -> bool {
    let creds = state.webauthn_credentials.lock().unwrap();
    !creds.passkeys.is_empty()
}

// --- Registration ---

/// Start WebAuthn registration ceremony.
/// Returns a JSON challenge for the browser to pass to navigator.credentials.create().
pub async fn register_start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let webauthn = match &state.webauthn {
        Some(w) => w,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "webauthn_not_configured"})),
            )
                .into_response()
        }
    };

    // Single admin user — fixed UUID
    let user_id = Uuid::new_v4();
    let user_name = "admin";
    let user_display = "Panorama Admin";

    let exclude = {
        let creds = state.webauthn_credentials.lock().unwrap();
        creds.passkeys.iter().map(|p| p.cred_id().clone()).collect::<Vec<_>>()
    };

    match webauthn.start_passkey_registration(user_id, user_name, user_display, Some(exclude)) {
        Ok((ccr, reg_state)) => {
            // Store registration state for finish step
            *state.webauthn_reg_state.lock().unwrap() = Some(reg_state);
            Json(serde_json::json!(ccr)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "WebAuthn registration start failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "registration_start_failed"})),
            )
                .into_response()
        }
    }
}

/// Finish WebAuthn registration ceremony.
/// Browser sends the credential response from navigator.credentials.create().
pub async fn register_finish(
    State(state): State<Arc<AppState>>,
    Json(reg): Json<RegisterPublicKeyCredential>,
) -> impl IntoResponse {
    let webauthn = match &state.webauthn {
        Some(w) => w,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "webauthn_not_configured"})),
            )
                .into_response()
        }
    };

    let reg_state = {
        let mut lock = state.webauthn_reg_state.lock().unwrap();
        match lock.take() {
            Some(s) => s,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "no_pending_registration"})),
                )
                    .into_response()
            }
        }
    };

    match webauthn.finish_passkey_registration(&reg, &reg_state) {
        Ok(passkey) => {
            let mut creds = state.webauthn_credentials.lock().unwrap();
            creds.passkeys.push(passkey);

            if let Some(ref path) = state.webauthn_credentials_path {
                if let Err(e) = save_credentials(path, &creds) {
                    tracing::error!(error = %e, "Failed to save WebAuthn credentials");
                }
            }

            tracing::info!("WebAuthn credential registered successfully");
            Json(serde_json::json!({"status": "registered"})).into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "WebAuthn registration finish failed");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "registration_failed", "detail": e.to_string()})),
            )
                .into_response()
        }
    }
}

// --- Authentication ---

/// Start WebAuthn authentication ceremony.
/// Returns a JSON challenge for the browser to pass to navigator.credentials.get().
pub async fn auth_start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let webauthn = match &state.webauthn {
        Some(w) => w,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "webauthn_not_configured"})),
            )
                .into_response()
        }
    };

    let allow_credentials = {
        let creds = state.webauthn_credentials.lock().unwrap();
        if creds.passkeys.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "no_credentials_registered"})),
            )
                .into_response();
        }
        creds.passkeys.clone()
    };

    match webauthn.start_passkey_authentication(&allow_credentials) {
        Ok((rcr, auth_state)) => {
            *state.webauthn_auth_state.lock().unwrap() = Some(auth_state);
            Json(serde_json::json!(rcr)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "WebAuthn auth start failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "auth_start_failed"})),
            )
                .into_response()
        }
    }
}

/// Finish WebAuthn authentication ceremony.
/// Browser sends the assertion response from navigator.credentials.get().
pub async fn auth_finish(
    State(state): State<Arc<AppState>>,
    Json(auth): Json<PublicKeyCredential>,
) -> impl IntoResponse {
    let webauthn = match &state.webauthn {
        Some(w) => w,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "webauthn_not_configured"})),
            )
                .into_response()
        }
    };

    let auth_state = {
        let mut lock = state.webauthn_auth_state.lock().unwrap();
        match lock.take() {
            Some(s) => s,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "no_pending_authentication"})),
                )
                    .into_response()
            }
        }
    };

    match webauthn.finish_passkey_authentication(&auth, &auth_state) {
        Ok(result) => {
            // Update credential counter to prevent cloning attacks
            let mut creds = state.webauthn_credentials.lock().unwrap();
            for passkey in &mut creds.passkeys {
                passkey.update_credential(&result);
            }
            if let Some(ref path) = state.webauthn_credentials_path {
                if let Err(e) = save_credentials(path, &creds) {
                    tracing::error!(error = %e, "Failed to update credential counters");
                }
            }

            tracing::info!("WebAuthn authentication successful");
            (
                StatusCode::OK,
                [
                    ("set-cookie", "admin_session=active; Path=/; HttpOnly; SameSite=Strict"),
                ],
                Json(serde_json::json!({"status": "authenticated"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "WebAuthn authentication failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "authentication_failed"})),
            )
                .into_response()
        }
    }
}

// --- Pages ---

/// Registration page — shown when no credentials exist yet.
pub async fn register_page(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if has_credentials(&state) {
        return (StatusCode::SEE_OTHER, [("location", "/login")], "").into_response();
    }
    Html(REGISTER_HTML).into_response()
}

const REGISTER_HTML: &str = r##"<!DOCTYPE html>
<html><head><title>Panorama — Register Security Key</title>
<style>
body { font-family: system-ui; display: flex; justify-content: center; align-items: center;
       height: 100vh; margin: 0; background: #1a1a2e; color: #e0e0e0; }
.card { background: #16213e; padding: 2rem; border-radius: 8px; max-width: 400px; text-align: center; }
button { margin-top: 1rem; padding: 0.75rem 2rem; background: #e94560; color: #fff;
         border: none; border-radius: 4px; cursor: pointer; font-size: 1rem; }
button:disabled { opacity: 0.5; cursor: not-allowed; }
.status { margin-top: 1rem; min-height: 1.5rem; }
.ok { color: #4ecca3; }
.err { color: #e94560; }
</style></head><body>
<div class="card">
  <h2>Register Security Key</h2>
  <p>Insert your YubiKey and press the button below to register it as the admin credential.</p>
  <button id="register-btn" onclick="startRegistration()">Register YubiKey</button>
  <div class="status" id="status"></div>
</div>
<script>
async function startRegistration() {
  const btn = document.getElementById('register-btn');
  const status = document.getElementById('status');
  btn.disabled = true;
  status.textContent = 'Starting registration...';
  status.className = 'status';
  try {
    const startResp = await fetch('/auth/webauthn/register/start', { method: 'POST' });
    if (!startResp.ok) throw new Error('Server rejected registration start');
    const options = await startResp.json();
    // Decode base64url fields for the browser API
    options.publicKey.challenge = base64urlToBuffer(options.publicKey.challenge);
    options.publicKey.user.id = base64urlToBuffer(options.publicKey.user.id);
    if (options.publicKey.excludeCredentials) {
      options.publicKey.excludeCredentials = options.publicKey.excludeCredentials.map(c => ({
        ...c, id: base64urlToBuffer(c.id)
      }));
    }
    status.textContent = 'Touch your security key...';
    const credential = await navigator.credentials.create(options);
    const finishResp = await fetch('/auth/webauthn/register/finish', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        id: credential.id,
        rawId: bufferToBase64url(credential.rawId),
        type: credential.type,
        response: {
          attestationObject: bufferToBase64url(credential.response.attestationObject),
          clientDataJSON: bufferToBase64url(credential.response.clientDataJSON),
        }
      })
    });
    if (!finishResp.ok) throw new Error('Registration failed');
    status.textContent = 'Key registered! Redirecting...';
    status.className = 'status ok';
    setTimeout(() => window.location.href = '/login', 1000);
  } catch (e) {
    status.textContent = 'Error: ' + e.message;
    status.className = 'status err';
    btn.disabled = false;
  }
}
function base64urlToBuffer(b64) {
  const pad = b64.length % 4 === 0 ? '' : '='.repeat(4 - b64.length % 4);
  const base64 = b64.replace(/-/g, '+').replace(/_/g, '/') + pad;
  const bin = atob(base64);
  return Uint8Array.from(bin, c => c.charCodeAt(0)).buffer;
}
function bufferToBase64url(buf) {
  const bytes = new Uint8Array(buf);
  let str = '';
  for (const b of bytes) str += String.fromCharCode(b);
  return btoa(str).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}
</script>
</body></html>
"##;
