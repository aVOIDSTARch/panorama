//! MCP server — JSON-RPC over stdio transport.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::protocol::*;
use crate::tools;
use cortex_core::ServiceManifest;

pub struct McpServer {
    cortex_url: String,
    http: reqwest::Client,
    tool_definitions: Vec<ToolDefinition>,
}

impl McpServer {
    pub fn new(manifest: ServiceManifest, cortex_url: String) -> Self {
        let tool_definitions = tools::build_tool_definitions(&manifest);
        tracing::info!(
            tools = tool_definitions.len(),
            "MCP server initialized with {} tools",
            tool_definitions.len()
        );

        Self {
            cortex_url,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            tool_definitions,
        }
    }

    /// Run the MCP server on stdin/stdout.
    pub async fn run(&self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                // EOF — client disconnected
                tracing::info!("stdin closed, shutting down");
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                Ok(req) => self.handle_request(req).await,
                Err(e) => JsonRpcResponse::error(
                    None,
                    -32700,
                    format!("Parse error: {e}"),
                ),
            };

            let mut output = serde_json::to_string(&response)?;
            output.push('\n');
            stdout.write_all(output.as_bytes()).await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        match req.method.as_str() {
            "initialize" => self.handle_initialize(req.id),
            "notifications/initialized" => {
                // Client acknowledgment — no response needed for notifications,
                // but since we read it as a request, just return empty success
                JsonRpcResponse::success(req.id, serde_json::json!({}))
            }
            "tools/list" => self.handle_tools_list(req.id),
            "tools/call" => self.handle_tools_call(req.id, req.params).await,
            "ping" => JsonRpcResponse::success(req.id, serde_json::json!({})),
            _ => JsonRpcResponse::method_not_found(req.id, &req.method),
        }
    }

    fn handle_initialize(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "cortex-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    fn handle_tools_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            serde_json::json!({
                "tools": self.tool_definitions
            }),
        )
    }

    async fn handle_tools_call(
        &self,
        id: Option<serde_json::Value>,
        params: serde_json::Value,
    ) -> JsonRpcResponse {
        let call: ToolCallParams = match serde_json::from_value(params) {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    -32602,
                    format!("Invalid params: {e}"),
                );
            }
        };

        let result = tools::execute_tool(
            &self.http,
            &self.cortex_url,
            &self.tool_definitions,
            &call.name,
            &call.arguments,
        )
        .await;

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap_or_default())
    }
}
