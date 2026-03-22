/// Analog communications runtime configuration.
#[derive(Clone)]
pub struct AnalogConfig {
    pub port: u16,
    pub cloak_url: String,
    pub cloak_manifest_token: String,
    pub cortex_url: String,
    /// Telnyx webhook signing public key (Ed25519).
    pub telnyx_public_key: Option<String>,
    /// Allowed sender phone numbers (E.164 format).
    pub allowed_senders: Vec<String>,
    /// Owner phone number for TOTP-protected commands.
    pub owner_number: Option<String>,
    /// TOTP shared secret (base32-encoded) for owner verification.
    pub owner_totp_secret: Option<String>,
    /// Datastore URL for recognized sender persistence.
    pub datastore_url: Option<String>,
}

impl AnalogConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let allowed = std::env::var("ANALOG_ALLOWED_SENDERS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        Ok(Self {
            port: std::env::var("ANALOG_PORT")
                .unwrap_or_else(|_| "8500".into())
                .parse()
                .unwrap_or(8500),
            cloak_url: std::env::var("CLOAK_URL")
                .unwrap_or_else(|_| "http://localhost:8300".into()),
            cloak_manifest_token: std::env::var("CLOAK_MANIFEST_TOKEN").unwrap_or_default(),
            cortex_url: std::env::var("CORTEX_URL")
                .unwrap_or_else(|_| "http://localhost:9000".into()),
            telnyx_public_key: std::env::var("TELNYX_PUBLIC_KEY").ok(),
            allowed_senders: allowed,
            owner_number: std::env::var("ANALOG_OWNER_NUMBER").ok(),
            owner_totp_secret: std::env::var("OWNER_TOTP_SECRET").ok(),
            datastore_url: std::env::var("DATASTORE_URL").ok(),
        })
    }
}
