use crate::config::SanitizerConfig;
use regex::RegexSet;

pub struct SanitizationRuleSet {
    pub injection_patterns: Option<RegexSet>,
    pub credential_patterns: Option<RegexSet>,
}

impl SanitizationRuleSet {
    pub fn compile(config: &SanitizerConfig) -> anyhow::Result<Self> {
        let injection_patterns = if config.injection_patterns.is_empty() {
            None
        } else {
            Some(
                RegexSet::new(&config.injection_patterns)
                    .map_err(|e| anyhow::anyhow!("invalid injection pattern: {e}"))?,
            )
        };

        let credential_patterns = if config.credential_patterns.is_empty() {
            None
        } else {
            Some(
                RegexSet::new(&config.credential_patterns)
                    .map_err(|e| anyhow::anyhow!("invalid credential pattern: {e}"))?,
            )
        };

        Ok(Self {
            injection_patterns,
            credential_patterns,
        })
    }

    pub fn check_injection(&self, text: &str) -> Option<String> {
        if let Some(ref patterns) = self.injection_patterns {
            let matches: Vec<_> = patterns.matches(text).into_iter().collect();
            if !matches.is_empty() {
                return Some(format!("injection pattern detected (rules: {matches:?})"));
            }
        }
        None
    }

    pub fn check_credential_leak(&self, text: &str) -> Option<String> {
        if let Some(ref patterns) = self.credential_patterns {
            let matches: Vec<_> = patterns.matches(text).into_iter().collect();
            if !matches.is_empty() {
                return Some(format!(
                    "credential pattern detected in response (rules: {matches:?})"
                ));
            }
        }
        None
    }
}
