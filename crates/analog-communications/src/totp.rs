use hmac::{Hmac, Mac};
use regex::Regex;
use sha1::Sha1;
use std::sync::LazyLock;

type HmacSha1 = Hmac<Sha1>;

/// Standard TOTP parameters (RFC 6238 defaults).
const TOTP_DIGITS: u32 = 6;
const TOTP_PERIOD: u64 = 30;
/// Allow +-1 time step to account for clock drift.
const TOTP_SKEW: u64 = 1;

/// Regex to find a 6-digit code in an SMS body.
/// Matches a standalone 6-digit sequence (not part of a longer number).
static CODE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)(\d{6})(?:\s|$)").unwrap());

/// Extract a 6-digit TOTP code from an SMS body, if present.
pub fn extract_totp_code(body: &str) -> Option<&str> {
    CODE_REGEX.captures(body).map(|c| c.get(1).unwrap().as_str())
}

/// Verify a TOTP code against a base32-encoded shared secret.
///
/// Uses HMAC-SHA1 per RFC 6238, with a +-1 step skew window.
pub fn verify_totp(secret_b32: &str, code: &str) -> bool {
    let secret = match base32_decode(secret_b32) {
        Some(s) => s,
        None => {
            tracing::warn!("Invalid base32 TOTP secret");
            return false;
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let current_step = now / TOTP_PERIOD;

    for offset in 0..=TOTP_SKEW {
        if generate_totp(&secret, current_step - offset) == code {
            return true;
        }
        if offset > 0 && generate_totp(&secret, current_step + offset) == code {
            return true;
        }
    }

    false
}

/// Generate a TOTP code for a given time step (RFC 6238 / RFC 4226).
fn generate_totp(secret: &[u8], time_step: u64) -> String {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(&time_step.to_be_bytes());
    let result = mac.finalize().into_bytes();

    // Dynamic truncation (RFC 4226 section 5.4)
    let offset = (result[19] & 0x0f) as usize;
    let binary = ((result[offset] as u32 & 0x7f) << 24)
        | ((result[offset + 1] as u32) << 16)
        | ((result[offset + 2] as u32) << 8)
        | (result[offset + 3] as u32);

    let otp = binary % 10u32.pow(TOTP_DIGITS);
    format!("{otp:0>width$}", width = TOTP_DIGITS as usize)
}

/// Decode a base32 string (RFC 4648, no padding required).
fn base32_decode(input: &str) -> Option<Vec<u8>> {
    let input = input.trim().to_uppercase();
    let input = input.trim_end_matches('=');

    let mut bits = 0u64;
    let mut bit_count = 0u32;
    let mut output = Vec::new();

    for c in input.chars() {
        let val = match c {
            'A'..='Z' => c as u64 - b'A' as u64,
            '2'..='7' => c as u64 - b'2' as u64 + 26,
            _ => return None,
        };
        bits = (bits << 5) | val;
        bit_count += 5;

        if bit_count >= 8 {
            bit_count -= 8;
            output.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_totp_code() {
        assert_eq!(extract_totp_code("123456"), Some("123456"));
        assert_eq!(extract_totp_code("my code is 654321 thanks"), Some("654321"));
        assert_eq!(extract_totp_code("123456 idea for a new thing"), Some("123456"));
        assert_eq!(extract_totp_code("no code here"), None);
        assert_eq!(extract_totp_code("12345"), None); // too short
        assert_eq!(extract_totp_code("1234567"), None); // 7 digits, no standalone 6
    }

    #[test]
    fn test_base32_decode() {
        // Verify base32 decode produces correct bytes for a known input.
        // "ME" = base32 for "a" (0x61 = 01100001 → 01100 00100 → M E)
        let decoded = base32_decode("ME").unwrap();
        assert_eq!(decoded, b"a");
        // Invalid char returns None
        assert!(base32_decode("!!!").is_none());
    }

    #[test]
    fn test_generate_totp_known_vector() {
        // RFC 6238 test vector: secret "12345678901234567890" at time step 1
        let secret = b"12345678901234567890";
        let code = generate_totp(secret, 1);
        assert_eq!(code, "287082");
    }
}
