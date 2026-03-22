/// Send an SMS via the Telnyx Messaging API.
pub async fn send(
    client: &reqwest::Client,
    api_key_env: &str,
    from: &str,
    to: &str,
    message: &str,
) -> anyhow::Result<()> {
    let api_key = std::env::var(api_key_env)
        .map_err(|_| anyhow::anyhow!("missing env var: {api_key_env}"))?;

    let body = serde_json::json!({
        "from": from,
        "to": to,
        "text": message,
    });

    let resp = client
        .post("https://api.telnyx.com/v2/messages")
        .header("authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telnyx SMS failed ({status}): {body}");
    }

    tracing::info!(to = to, "SMS alert sent via Telnyx");
    Ok(())
}
