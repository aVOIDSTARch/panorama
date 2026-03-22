/// Sender identity levels — determines what actions an inbound message can trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityLevel {
    /// Owner (with TOTP verification) — can trigger any action.
    Owner,
    /// Known sender on the allowlist — can trigger IDEA_CAPTURE.
    Known,
    /// Recognized but unverified — messages quarantined for review.
    Recognized,
    /// Unknown sender — messages quarantined.
    Unknown,
}

/// Resolve a sender's identity level based on their phone number.
///
/// If the sender is the owner AND the message body contains a valid TOTP code,
/// identity is upgraded to `Owner`. Otherwise the owner gets `Known`.
pub fn resolve_sender(
    from: &str,
    body: &str,
    allowed_senders: &[String],
    owner_number: Option<&str>,
    owner_totp_secret: Option<&str>,
    recognized_senders: &[String],
) -> IdentityLevel {
    // Check if this is the owner
    if let Some(owner) = owner_number {
        if from == owner {
            // If TOTP secret is configured, try to verify the code in the message
            if let Some(secret) = owner_totp_secret {
                if let Some(code) = crate::totp::extract_totp_code(body) {
                    if crate::totp::verify_totp(secret, code) {
                        tracing::info!(from = %from, "Owner TOTP verified — full Owner access");
                        return IdentityLevel::Owner;
                    }
                    tracing::warn!(from = %from, "Owner sent invalid TOTP — downgrading to Known");
                }
            }
            // Owner number without valid TOTP gets Known level
            return IdentityLevel::Known;
        }
    }

    // Check allowlist
    if allowed_senders.iter().any(|s| s == from) {
        return IdentityLevel::Known;
    }

    // Check recognized sender list
    if recognized_senders.iter().any(|s| s == from) {
        return IdentityLevel::Recognized;
    }

    IdentityLevel::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_sender() {
        let level = resolve_sender("+15551234567", "hello", &[], None, None, &[]);
        assert_eq!(level, IdentityLevel::Unknown);
    }

    #[test]
    fn test_known_sender_on_allowlist() {
        let allowed = vec!["+15551234567".to_string()];
        let level = resolve_sender("+15551234567", "hello", &allowed, None, None, &[]);
        assert_eq!(level, IdentityLevel::Known);
    }

    #[test]
    fn test_owner_without_totp_gets_known() {
        let level = resolve_sender(
            "+15559999999",
            "hello",
            &[],
            Some("+15559999999"),
            None,
            &[],
        );
        assert_eq!(level, IdentityLevel::Known);
    }

    #[test]
    fn test_recognized_sender() {
        let recognized = vec!["+15551234567".to_string()];
        let level = resolve_sender("+15551234567", "hello", &[], None, None, &recognized);
        assert_eq!(level, IdentityLevel::Recognized);
    }
}
