use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

use cloak_core::{CloakError, TokenClaims};

type HmacSha256 = Hmac<Sha256>;

/// Generate a cryptographically random 256-bit signing key.
pub fn generate_signing_key() -> Vec<u8> {
    let mut key = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Sign token claims, producing: base64url(claims_json).hex(hmac_sha256)
///
/// This format is compatible with Episteme's Python `_verify_and_decode`.
pub fn sign_claims(claims: &TokenClaims, key: &[u8]) -> Result<String, CloakError> {
    let claims_json = serde_json::to_vec(claims)
        .map_err(|e| CloakError::Internal(format!("Failed to serialize claims: {e}")))?;
    let payload_b64 = URL_SAFE_NO_PAD.encode(&claims_json);

    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| CloakError::Internal(format!("Invalid signing key: {e}")))?;
    mac.update(payload_b64.as_bytes());
    let signature = hex::encode(&mac.finalize().into_bytes());

    Ok(format!("{payload_b64}.{signature}"))
}

/// Verify token signature and decode claims.
///
/// Token format: base64url(claims_json).hex(hmac_sha256)
/// Uses constant-time comparison for the signature check.
pub fn verify_and_decode(token: &str, key: &[u8]) -> Result<TokenClaims, CloakError> {
    let parts: Vec<&str> = token.rsplitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(CloakError::MalformedToken);
    }

    let signature_hex = parts[0];
    let payload_b64 = parts[1];

    // Compute expected signature
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| CloakError::Internal(format!("Invalid signing key: {e}")))?;
    mac.update(payload_b64.as_bytes());
    let expected = hex::encode(&mac.finalize().into_bytes());

    // Constant-time comparison
    if !constant_time_eq(expected.as_bytes(), signature_hex.as_bytes()) {
        return Err(CloakError::InvalidSignature);
    }

    // Decode claims
    let claims_json = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| CloakError::InvalidToken("Cannot decode payload".into()))?;

    serde_json::from_slice(&claims_json)
        .map_err(|e| CloakError::InvalidToken(format!("Cannot parse claims: {e}")))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// hex encoding helper (avoids pulling in another crate)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use cloak_core::{OperationClass, ServiceScope};

    fn test_claims() -> TokenClaims {
        TokenClaims {
            job_id: "job-42".into(),
            agent_class: "researcher".into(),
            issued_at: Utc::now(),
            expires_at: Utc::now(),
            services: vec![ServiceScope {
                service: "episteme".into(),
                operation_class: OperationClass::Read,
                resources: vec!["*".into()],
            }],
        }
    }

    #[test]
    fn test_sign_then_verify() {
        let key = generate_signing_key();
        let claims = test_claims();
        let token = sign_claims(&claims, &key).unwrap();
        let decoded = verify_and_decode(&token, &key).unwrap();
        assert_eq!(decoded.job_id, "job-42");
        assert_eq!(decoded.agent_class, "researcher");
    }

    #[test]
    fn test_tampered_payload() {
        let key = generate_signing_key();
        let claims = test_claims();
        let token = sign_claims(&claims, &key).unwrap();

        // Tamper with the payload
        let mut chars: Vec<char> = token.chars().collect();
        if chars[0] == 'a' {
            chars[0] = 'b';
        } else {
            chars[0] = 'a';
        }
        let tampered: String = chars.into_iter().collect();

        assert!(matches!(
            verify_and_decode(&tampered, &key),
            Err(CloakError::InvalidSignature) | Err(CloakError::InvalidToken(_))
        ));
    }

    #[test]
    fn test_wrong_key() {
        let key1 = generate_signing_key();
        let key2 = generate_signing_key();
        let claims = test_claims();
        let token = sign_claims(&claims, &key1).unwrap();
        assert!(matches!(
            verify_and_decode(&token, &key2),
            Err(CloakError::InvalidSignature)
        ));
    }

    #[test]
    fn test_malformed_token() {
        let key = generate_signing_key();
        assert!(matches!(
            verify_and_decode("notavalidtoken", &key),
            Err(CloakError::MalformedToken)
        ));
    }
}
