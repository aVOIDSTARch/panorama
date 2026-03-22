use cortex_core::ServiceManifest;
use cortex_mcp::server::McpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // MCP servers log to stderr (stdout is the JSON-RPC transport)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cortex_mcp=info".parse().unwrap()),
        )
        .init();

    let manifest_path =
        std::env::var("CORTEX_MANIFEST").unwrap_or_else(|_| "cortex-manifest.toml".into());
    let cortex_url =
        std::env::var("CORTEX_URL").unwrap_or_else(|_| "http://localhost:9000".into());

    let manifest = ServiceManifest::from_file(&manifest_path)
        .map_err(|e| anyhow::anyhow!("Failed to load manifest: {e}"))?;

    tracing::info!(
        services = manifest.services.len(),
        "Loaded manifest from {manifest_path}"
    );

    let server = McpServer::new(manifest, cortex_url);
    server.run().await
}
