use crate::sanitizer::rules::SanitizationRuleSet;
use crate::types::{SanitizationError, SanitizationErrorKind};

pub struct OutboundSanitizer {
    rules: SanitizationRuleSet,
}

impl OutboundSanitizer {
    pub fn new(rules: SanitizationRuleSet) -> Self {
        Self { rules }
    }

    /// Sanitize provider response text. Returns Ok(sanitized_text) or Err on credential leak.
    pub fn sanitize(&self, response_text: &str) -> Result<String, SanitizationError> {
        // 1. Credential scrub — defensive scan for API key patterns
        if let Some(detail) = self.rules.check_credential_leak(response_text) {
            return Err(SanitizationError {
                kind: SanitizationErrorKind::ContentViolation,
                field: "response".into(),
                detail,
            });
        }

        // 2. Return sanitized text (no modifications needed if no violations)
        Ok(response_text.to_string())
    }
}
