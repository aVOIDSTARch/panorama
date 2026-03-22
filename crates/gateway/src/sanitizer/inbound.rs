use crate::sanitizer::rules::SanitizationRuleSet;
use crate::types::{InboundRequest, SanitizationError, SanitizationErrorKind, SanitizedRequest};
use chrono::Utc;
use sha2::{Digest, Sha256};

pub struct InboundSanitizer {
    rules: SanitizationRuleSet,
    max_prompt_bytes: usize,
}

impl InboundSanitizer {
    pub fn new(rules: SanitizationRuleSet, max_prompt_bytes: usize) -> Self {
        Self {
            rules,
            max_prompt_bytes,
        }
    }

    pub fn sanitize(
        &self,
        request: &InboundRequest,
    ) -> Result<SanitizedRequest, SanitizationError> {
        // 1. Encoding check — String type in Rust guarantees UTF-8, but check for null bytes
        if request.prompt.contains('\0') {
            return Err(SanitizationError {
                kind: SanitizationErrorKind::EncodingError,
                field: "prompt".into(),
                detail: "prompt contains null bytes".into(),
            });
        }

        // 2. Size check
        if request.prompt.len() > self.max_prompt_bytes {
            return Err(SanitizationError {
                kind: SanitizationErrorKind::SizeLimitExceeded,
                field: "prompt".into(),
                detail: format!(
                    "prompt size {} bytes exceeds limit {} bytes",
                    request.prompt.len(),
                    self.max_prompt_bytes
                ),
            });
        }

        // 3. Empty prompt check
        if request.prompt.trim().is_empty() {
            return Err(SanitizationError {
                kind: SanitizationErrorKind::SchemaViolation,
                field: "prompt".into(),
                detail: "prompt is empty".into(),
            });
        }

        // 4. Injection pattern scan
        if let Some(detail) = self.rules.check_injection(&request.prompt) {
            return Err(SanitizationError {
                kind: SanitizationErrorKind::InjectionPattern,
                field: "prompt".into(),
                detail,
            });
        }

        // 5. Compute SHA-256 hash of sanitized prompt
        let mut hasher = Sha256::new();
        hasher.update(request.prompt.as_bytes());
        let inbound_hash = format!("{:x}", hasher.finalize());

        Ok(SanitizedRequest {
            request_id: request.request_id,
            route_key: request.route_key.clone(),
            prompt: request.prompt.clone(),
            caller_id: request.caller_metadata.caller_id.clone(),
            session_id: request.caller_metadata.session_id.clone(),
            options: request.options.clone(),
            inbound_hash,
            received_at: Utc::now(),
        })
    }
}
