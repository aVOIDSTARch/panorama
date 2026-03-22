use crate::types::Alert;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Send an alert to a webhook endpoint with HMAC-SHA256 signature.
pub async fn send(
    client: &reqwest::Client,
    url: &str,
    secret_env: &str,
    alert: &Alert,
) -> anyhow::Result<()> {
    let payload = serde_json::to_string(alert)?;

    // Compute HMAC signature
    let secret = std::env::var(secret_env).unwrap_or_default();
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload.as_bytes());
    let signature = hex_encode(mac.finalize().into_bytes());

    let resp = client
        .post(url)
        .header("content-type", "application/json")
        .header("x-signature-256", format!("sha256={signature}"))
        .body(payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("webhook failed ({status}): {body}");
    }

    Ok(())
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .fold(String::new(), |mut s, b| {
            use std::fmt::Write;
            write!(s, "{b:02x}").unwrap();
            s
        })
}
