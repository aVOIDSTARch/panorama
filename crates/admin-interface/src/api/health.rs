use axum::extract::State;
use axum::response::Html;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use webauthn_rs::prelude::*;

use crate::auth::webauthn::StoredCredentials;

/// Check health of all downstream services and return an HTMX fragment.
pub async fn health_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    let services = [
        ("Cloak", &state.cloak_url),
        ("Cortex", &state.cortex_url),
        ("Episteme", &state.episteme_url),
        ("Cerebro", &state.cerebro_url),
        ("Datastore", &state.datastore_url),
        ("Gateway", &state.gateway_url),
    ];

    let mut rows = String::new();
    for (name, url) in &services {
        let health_url = format!("{url}/health");
        let (status_class, status_text) = match state.http.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => ("status-ok", "healthy"),
            Ok(_) => ("status-degraded", "degraded"),
            Err(_) => ("status-down", "unreachable"),
        };
        rows.push_str(&format!(
            r#"<tr><td>{name}</td><td class="{status_class}">{status_text}</td><td><code>{url}</code></td></tr>"#,
        ));
    }

    Html(format!(
        r#"<table><thead><tr><th>Service</th><th>Status</th><th>URL</th></tr></thead><tbody>{rows}</tbody></table>"#,
    ))
}

/// Return list of Cloak-registered services as an HTMX fragment.
pub async fn services_panel(State(state): State<Arc<AppState>>) -> Html<String> {
    let url = format!("{}/cloak/services", state.cloak_url);
    match state.http.get(&url).send().await {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let services = body.as_array().cloned().unwrap_or_default();
            let mut rows = String::new();
            for svc in &services {
                let id = svc.get("service_id").and_then(|v| v.as_str()).unwrap_or("?");
                let stype = svc.get("service_type").and_then(|v| v.as_str()).unwrap_or("?");
                let version = svc.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                rows.push_str(&format!(
                    "<tr><td>{id}</td><td>{stype}</td><td>{version}</td></tr>"
                ));
            }
            Html(format!(
                r#"<table><thead><tr><th>Service</th><th>Type</th><th>Version</th></tr></thead><tbody>{rows}</tbody></table>"#,
            ))
        }
        Err(e) => Html(format!("<p class='status-down'>Cannot reach Cloak: {e}</p>")),
    }
}

pub struct AppState {
    pub http: reqwest::Client,
    pub admin_password: String,
    pub cloak_url: String,
    pub cortex_url: String,
    pub episteme_url: String,
    pub cerebro_url: String,
    pub datastore_url: String,
    pub gateway_url: String,
    pub log_db: Option<Mutex<Connection>>,
    // WebAuthn (FIDO2 / YubiKey)
    pub webauthn: Option<Webauthn>,
    pub webauthn_credentials: Mutex<StoredCredentials>,
    pub webauthn_credentials_path: Option<String>,
    pub webauthn_reg_state: Mutex<Option<PasskeyRegistration>>,
    pub webauthn_auth_state: Mutex<Option<PasskeyAuthentication>>,
}

impl AppState {
    pub fn from_env() -> Self {
        let log_db_path = std::env::var("LOG_DB_PATH")
            .unwrap_or_else(|_| "data/panorama_logs.db".into());

        let log_db = Connection::open_with_flags(
            &log_db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()
        .map(Mutex::new);

        if log_db.is_none() {
            tracing::warn!("Could not open log database at {log_db_path} — log/error panels will be empty");
        }

        // WebAuthn setup — requires WEBAUTHN_RP_ID and WEBAUTHN_RP_ORIGIN
        let (webauthn, creds_path) = match (
            std::env::var("WEBAUTHN_RP_ID"),
            std::env::var("WEBAUTHN_RP_ORIGIN"),
        ) {
            (Ok(rp_id), Ok(rp_origin)) => {
                match url::Url::parse(&rp_origin) {
                    Ok(origin_url) => {
                        match webauthn_rs::WebauthnBuilder::new(&rp_id, &origin_url) {
                            Ok(builder) => {
                                let wa = builder.rp_name("Panorama Admin").build();
                                match wa {
                                    Ok(wa) => {
                                        let path = std::env::var("WEBAUTHN_CREDENTIALS_PATH")
                                            .unwrap_or_else(|_| "data/webauthn_credentials.json".into());
                                        tracing::info!("WebAuthn enabled (RP: {rp_id})");
                                        (Some(wa), Some(path))
                                    }
                                    Err(e) => {
                                        tracing::warn!("WebAuthn build failed: {e} — falling back to password auth");
                                        (None, None)
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("WebAuthn builder failed: {e} — falling back to password auth");
                                (None, None)
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Invalid WEBAUTHN_RP_ORIGIN: {e} — falling back to password auth");
                        (None, None)
                    }
                }
            }
            _ => {
                tracing::info!("WebAuthn not configured (set WEBAUTHN_RP_ID and WEBAUTHN_RP_ORIGIN to enable)");
                (None, None)
            }
        };

        let stored_creds = creds_path
            .as_deref()
            .map(crate::auth::webauthn::load_credentials)
            .unwrap_or_default();

        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("failed to build HTTP client"),
            // Populated from env as fallback; overwritten from Cloak at startup
            admin_password: std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "panorama".into()),
            cloak_url: std::env::var("CLOAK_URL").unwrap_or_else(|_| "http://localhost:8300".into()),
            cortex_url: std::env::var("CORTEX_URL").unwrap_or_else(|_| "http://localhost:9000".into()),
            episteme_url: std::env::var("EPISTEME_URL").unwrap_or_else(|_| "http://localhost:8100".into()),
            cerebro_url: std::env::var("CEREBRO_URL").unwrap_or_else(|_| "http://localhost:8101".into()),
            datastore_url: std::env::var("DATASTORE_URL").unwrap_or_else(|_| "http://localhost:8102".into()),
            gateway_url: std::env::var("GATEWAY_URL").unwrap_or_else(|_| "http://localhost:8800".into()),
            log_db,
            webauthn,
            webauthn_credentials: Mutex::new(stored_creds),
            webauthn_credentials_path: creds_path,
            webauthn_reg_state: Mutex::new(None),
            webauthn_auth_state: Mutex::new(None),
        }
    }
}
