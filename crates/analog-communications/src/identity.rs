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
pub fn resolve_sender(
    from: &str,
    allowed_senders: &[String],
    owner_number: Option<&str>,
) -> IdentityLevel {
    // Check if this is the owner
    if let Some(owner) = owner_number {
        if from == owner {
            // TODO: Check TOTP token in message for full Owner level
            // For now, owner number gets Known level (TOTP required for Owner)
            return IdentityLevel::Known;
        }
    }

    // Check allowlist
    if allowed_senders.iter().any(|s| s == from) {
        return IdentityLevel::Known;
    }

    // TODO: Check recognized list (senders who have been seen before)

    IdentityLevel::Unknown
}
