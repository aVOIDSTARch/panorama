use regex::Regex;
use std::sync::LazyLock;

use crate::inbound::SanitizedMessage;

static E164_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\+[1-9]\d{1,14}$").unwrap());

static CONTROL_CHARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]").unwrap());

const MAX_SMS_LENGTH: usize = 1600; // Telnyx max for concatenated SMS

/// Sanitize an inbound SMS message.
///
/// Checks:
/// - E.164 format for sender number
/// - Message length limits
/// - Control character stripping
/// - Empty body rejection
pub fn sanitize_sms(from: &str, body: &str) -> Result<SanitizedMessage, SanitizationError> {
    // Validate sender E.164 format
    if !E164_REGEX.is_match(from) {
        return Err(SanitizationError::InvalidSender(from.to_string()));
    }

    // Strip control characters
    let clean_body = CONTROL_CHARS.replace_all(body, "").to_string();
    let clean_body = clean_body.trim().to_string();

    // Reject empty
    if clean_body.is_empty() {
        return Err(SanitizationError::EmptyBody);
    }

    // Length check
    if clean_body.len() > MAX_SMS_LENGTH {
        return Err(SanitizationError::TooLong {
            actual: clean_body.len(),
            max: MAX_SMS_LENGTH,
        });
    }

    // Extract labels (hashtag-style markers in the message)
    let labels: Vec<String> = clean_body
        .split_whitespace()
        .filter(|w| w.starts_with('#') && w.len() > 1)
        .map(|w| w[1..].to_lowercase())
        .collect();

    Ok(SanitizedMessage {
        from: from.to_string(),
        body: clean_body,
        received_at: chrono::Utc::now().to_rfc3339(),
        message_id: uuid::Uuid::new_v4().to_string(),
        labels,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum SanitizationError {
    #[error("Invalid sender number (not E.164): {0}")]
    InvalidSender(String),
    #[error("Empty message body")]
    EmptyBody,
    #[error("Message too long ({actual} bytes, max {max})")]
    TooLong { actual: usize, max: usize },
}
