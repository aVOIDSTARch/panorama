use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use std::sync::Arc;

use crate::api::health::AppState;

/// Login page — shows WebAuthn button when configured, password form as fallback.
pub async fn login_page(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // If WebAuthn is configured but no credentials registered, redirect to registration
    if state.webauthn.is_some() && !super::webauthn::has_credentials(&state) {
        return (StatusCode::SEE_OTHER, [("location", "/auth/register")], "").into_response();
    }

    let webauthn_enabled = state.webauthn.is_some() && super::webauthn::has_credentials(&state);
    let html = build_login_html(webauthn_enabled);
    Html(html).into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub password: String,
}

/// Handle login form submission (password fallback).
pub async fn login_submit(Form(form): Form<LoginForm>) -> impl IntoResponse {
    let expected = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "panorama".into());

    if form.password == expected {
        (
            StatusCode::SEE_OTHER,
            [
                ("location", "/"),
                (
                    "set-cookie",
                    "admin_session=active; Path=/; HttpOnly; SameSite=Strict",
                ),
            ],
            "",
        )
            .into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Html("<h2>Invalid password</h2><a href='/login'>Try again</a>")).into_response()
    }
}

fn build_login_html(webauthn_enabled: bool) -> String {
    let webauthn_section = if webauthn_enabled {
        r##"<div class="divider">Security Key</div>
<button id="webauthn-btn" type="button" onclick="startAuth()">Authenticate with YubiKey</button>
<div class="status" id="status"></div>
<div class="divider">Or use password</div>
<script>
async function startAuth() {
  const btn = document.getElementById('webauthn-btn');
  const status = document.getElementById('status');
  btn.disabled = true;
  status.textContent = 'Starting authentication...';
  status.className = 'status';
  try {
    const startResp = await fetch('/auth/webauthn/auth/start', { method: 'POST' });
    if (!startResp.ok) throw new Error('Server rejected auth start');
    const options = await startResp.json();
    options.publicKey.challenge = base64urlToBuffer(options.publicKey.challenge);
    if (options.publicKey.allowCredentials) {
      options.publicKey.allowCredentials = options.publicKey.allowCredentials.map(c => ({
        ...c, id: base64urlToBuffer(c.id)
      }));
    }
    status.textContent = 'Touch your security key...';
    const assertion = await navigator.credentials.get(options);
    const finishResp = await fetch('/auth/webauthn/auth/finish', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        id: assertion.id,
        rawId: bufferToBase64url(assertion.rawId),
        type: assertion.type,
        response: {
          authenticatorData: bufferToBase64url(assertion.response.authenticatorData),
          clientDataJSON: bufferToBase64url(assertion.response.clientDataJSON),
          signature: bufferToBase64url(assertion.response.signature),
          userHandle: assertion.response.userHandle ? bufferToBase64url(assertion.response.userHandle) : null,
        }
      })
    });
    if (!finishResp.ok) throw new Error('Authentication failed');
    window.location.href = '/';
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
</script>"##
    } else {
        ""
    };

    format!(
        r##"<!DOCTYPE html>
<html><head><title>Panorama Admin</title>
<style>body{{font-family:system-ui;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#1a1a2e}}
form,.card{{background:#16213e;padding:2rem;border-radius:8px;color:#e0e0e0;max-width:320px;width:100%}}
input{{display:block;margin:0.5rem 0;padding:0.5rem;width:200px;border:1px solid #555;background:#0f3460;color:#fff;border-radius:4px}}
button{{margin-top:1rem;padding:0.5rem 1.5rem;background:#e94560;color:#fff;border:none;border-radius:4px;cursor:pointer;width:100%}}
button:disabled{{opacity:0.5;cursor:not-allowed}}
.divider{{margin:1rem 0 0.5rem;text-align:center;color:#888;font-size:0.85rem}}
.status{{margin-top:0.5rem;min-height:1.2rem;font-size:0.85rem;text-align:center}}
.ok{{color:#4ecca3}}.err{{color:#e94560}}
</style></head><body>
<form method="post" action="/login">
<h2 style="text-align:center">Panorama Admin</h2>
{webauthn_section}
<label>Password</label>
<input type="password" name="password" autofocus>
<button type="submit">Login</button>
</form></body></html>"##
    )
}
