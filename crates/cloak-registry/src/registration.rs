use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;
use uuid::Uuid;

use cloak_core::{CloakError, RegistrationRequest, RegistrationResponse};
use cloak_tokens::signing::generate_signing_key;

use crate::store::{RegisteredService, ServiceStore};

/// Handle a service registration request.
///
/// 1. Validate the manifest token (bearer auth — caller provides pre-provisioned token)
/// 2. Generate a session ID (UUID) and a per-session HMAC signing key
/// 3. Store in the ServiceStore
/// 4. Return session_id, signing_key (base64), and halt_stream_url
pub fn handle_register(
    store: &ServiceStore,
    req: RegistrationRequest,
    server_port: u16,
) -> Result<(RegistrationResponse, Vec<u8>), CloakError> {
    let session_id = Uuid::new_v4().to_string();
    let signing_key = generate_signing_key();

    let service = RegisteredService {
        manifest: req.clone(),
        session_id: session_id.clone(),
        signing_key: signing_key.clone(),
        registered_at: Utc::now(),
    };

    // Register and create SSE channel
    let _rx = store.register(service);

    let halt_stream_url = format!(
        "http://localhost:{}/cloak/services/{}/halt-stream",
        server_port, req.service_id
    );

    let resp = RegistrationResponse {
        session_id,
        signing_key: STANDARD.encode(&signing_key),
        halt_stream_url,
    };

    Ok((resp, signing_key))
}
