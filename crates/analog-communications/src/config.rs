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

    /// Fetch secrets that live in Infisical (via Cloak) and override any env
    /// fallback values. Non-fatal — if Cloak is unreachable the values set by
    /// `from_env` are kept unchanged.
    pub async fn load_cloak_secrets(&mut self, http: &reqwest::Client) {
        async fn fetch(http: &reqwest::Client, cloak_url: &str, key: &str) -> Option<String> {
            let url = format!("{}/cloak/secrets/{}", cloak_url, key);
            let resp = http.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let body: serde_json::Value = resp.json().await.ok()?;
            body.get("value")?.as_str().map(String::from)
        }

        if let Some(val) = fetch(http, &self.cloak_url, "TELNYX_PUBLIC_KEY").await {
            tracing::info!("Loaded TELNYX_PUBLIC_KEY from Cloak");
            self.telnyx_public_key = Some(val);
        }
        if let Some(val) = fetch(http, &self.cloak_url, "OWNER_TOTP_SECRET").await {
            tracing::info!("Loaded OWNER_TOTP_SECRET from Cloak");
            self.owner_totp_secret = Some(val);
        }
    }
}
