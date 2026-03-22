//! Tool registry — auto-generated from cortex-manifest.toml + service-specific enrichments.

use crate::protocol::{ContentBlock, ToolCallResult, ToolDefinition};
use cortex_core::ServiceManifest;
use serde_json::json;

/// Build all tool definitions from the manifest + hardcoded enrichments.
pub fn build_tool_definitions(manifest: &ServiceManifest) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    // Generic proxy tools for each manifest service
    for (key, config) in &manifest.services {
        tools.push(ToolDefinition {
            name: format!("{key}_request"),
            description: format!(
                "Send an HTTP request to the {} service via Cortex proxy. \
                 The request is forwarded to {}/{key}/{{path}}.",
                config.name, "Cortex"
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "method": {
                        "type": "string",
                        "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"],
                        "description": "HTTP method"
                    },
                    "path": {
                        "type": "string",
                        "description": format!("Path after /{key}/ (e.g. 'health', 'api/search')")
                    },
                    "body": {
                        "type": "object",
                        "description": "JSON body (for POST/PUT/PATCH)"
                    },
                    "query": {
                        "type": "object",
                        "description": "Query string parameters as key-value pairs",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "required": ["method", "path"]
            }),
        });
    }

    // Service-specific enriched tools
    tools.extend(cerebro_tools());
    tools.extend(episteme_tools());
    tools.extend(datastore_tools());

    tools
}

/// Execute a tool call.
pub async fn execute_tool(
    http: &reqwest::Client,
    cortex_url: &str,
    tools: &[ToolDefinition],
    name: &str,
    arguments: &serde_json::Value,
) -> ToolCallResult {
    // Check if it's a generic proxy tool
    if name.ends_with("_request") {
        let service_key = name.trim_end_matches("_request");
        return execute_proxy_request(http, cortex_url, service_key, arguments).await;
    }

    // Check enriched tools
    match name {
        "cerebro_search" => execute_enriched(http, cortex_url, "cerebro", "POST", "api/search", arguments).await,
        "cerebro_ingest" => execute_enriched(http, cortex_url, "cerebro", "POST", "quarantine/ingest", arguments).await,
        "cerebro_query" => execute_enriched(http, cortex_url, "cerebro", "POST", "api/query", arguments).await,
        "episteme_list_projects" => execute_enriched(http, cortex_url, "episteme", "GET", "api/projects", arguments).await,
        "episteme_get_document" => {
            let project = arguments.get("project").and_then(|v| v.as_str()).unwrap_or("");
            let doc_id = arguments.get("document_id").and_then(|v| v.as_str()).unwrap_or("");
            let path = format!("api/projects/{project}/documents/{doc_id}");
            execute_enriched(http, cortex_url, "episteme", "GET", &path, arguments).await
        }
        "episteme_search" => execute_enriched(http, cortex_url, "episteme", "POST", "api/search", arguments).await,
        "datastore_query" => execute_enriched(http, cortex_url, "datastore", "POST", "query", arguments).await,
        "datastore_upsert" => execute_enriched(http, cortex_url, "datastore", "POST", "upsert", arguments).await,
        "datastore_delete" => execute_enriched(http, cortex_url, "datastore", "POST", "delete", arguments).await,
        _ => {
            // Check tool exists
            if tools.iter().any(|t| t.name == name) {
                error_result(format!("Tool '{name}' exists but has no handler"))
            } else {
                error_result(format!("Unknown tool: {name}"))
            }
        }
    }
}

/// Execute a generic proxy request through Cortex.
async fn execute_proxy_request(
    http: &reqwest::Client,
    cortex_url: &str,
    service_key: &str,
    arguments: &serde_json::Value,
) -> ToolCallResult {
    let method = arguments
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET");
    let path = arguments
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let body = arguments.get("body");

    let mut url = format!("{cortex_url}/{service_key}/{path}");

    // Add query params
    if let Some(query) = arguments.get("query").and_then(|v| v.as_object()) {
        let params: Vec<String> = query
            .iter()
            .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
            .collect();
        if !params.is_empty() {
            url = format!("{url}?{}", params.join("&"));
        }
    }

    let request = match method.to_uppercase().as_str() {
        "GET" => http.get(&url),
        "POST" => {
            let r = http.post(&url);
            if let Some(b) = body { r.json(b) } else { r }
        }
        "PUT" => {
            let r = http.put(&url);
            if let Some(b) = body { r.json(b) } else { r }
        }
        "PATCH" => {
            let r = http.patch(&url);
            if let Some(b) = body { r.json(b) } else { r }
        }
        "DELETE" => http.delete(&url),
        _ => return error_result(format!("Unsupported method: {method}")),
    };

    match request.send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                ToolCallResult {
                    content: vec![ContentBlock::text(body)],
                    is_error: None,
                }
            } else {
                ToolCallResult {
                    content: vec![ContentBlock::text(format!(
                        "HTTP {}: {}",
                        status.as_u16(),
                        body
                    ))],
                    is_error: Some(true),
                }
            }
        }
        Err(e) => error_result(format!("Request failed: {e}")),
    }
}

/// Execute an enriched tool (specific path on a service).
async fn execute_enriched(
    http: &reqwest::Client,
    cortex_url: &str,
    service: &str,
    method: &str,
    path: &str,
    arguments: &serde_json::Value,
) -> ToolCallResult {
    let url = format!("{cortex_url}/{service}/{path}");

    let request = match method {
        "GET" => http.get(&url),
        "POST" => http.post(&url).json(arguments),
        "PUT" => http.put(&url).json(arguments),
        "DELETE" => http.delete(&url).json(arguments),
        _ => return error_result(format!("Unsupported method: {method}")),
    };

    match request.send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                ToolCallResult {
                    content: vec![ContentBlock::text(body)],
                    is_error: None,
                }
            } else {
                ToolCallResult {
                    content: vec![ContentBlock::text(format!("HTTP {}: {}", status.as_u16(), body))],
                    is_error: Some(true),
                }
            }
        }
        Err(e) => error_result(format!("Request failed: {e}")),
    }
}

fn error_result(msg: String) -> ToolCallResult {
    ToolCallResult {
        content: vec![ContentBlock::text(msg)],
        is_error: Some(true),
    }
}

// --- Service-specific tool enrichments ---

fn cerebro_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "cerebro_search".into(),
            description: "Search the Cerebro knowledge graph for ideas, notes, and captured content.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Max results (default 20)" },
                    "labels": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Filter by labels"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "cerebro_ingest".into(),
            description: "Ingest a new idea or note into Cerebro's quarantine for processing.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Source identifier (e.g. 'mcp', 'sms', 'manual')" },
                    "content": { "type": "string", "description": "The content to ingest" },
                    "labels": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Labels/tags for categorization"
                    }
                },
                "required": ["source", "content"]
            }),
        },
        ToolDefinition {
            name: "cerebro_query".into(),
            description: "Run a structured query against Cerebro's knowledge graph.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "collection": { "type": "string", "description": "Collection to query" },
                    "filter": { "type": "object", "description": "Filter conditions" },
                    "limit": { "type": "integer", "description": "Max results" }
                },
                "required": ["collection"]
            }),
        },
    ]
}

fn episteme_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "episteme_list_projects".into(),
            description: "List all projects in Episteme.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "episteme_get_document".into(),
            description: "Retrieve a specific document from an Episteme project.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project name" },
                    "document_id": { "type": "string", "description": "Document ID" }
                },
                "required": ["project", "document_id"]
            }),
        },
        ToolDefinition {
            name: "episteme_search".into(),
            description: "Search across all Episteme documents using semantic or keyword search.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "project": { "type": "string", "description": "Limit to a specific project" },
                    "limit": { "type": "integer", "description": "Max results (default 10)" }
                },
                "required": ["query"]
            }),
        },
    ]
}

fn datastore_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "datastore_query".into(),
            description: "Query documents from a Datastore collection.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "collection": { "type": "string", "description": "Collection name" },
                    "query": { "type": "object", "description": "Query filter" },
                    "limit": { "type": "integer", "description": "Max results" }
                },
                "required": ["collection"]
            }),
        },
        ToolDefinition {
            name: "datastore_upsert".into(),
            description: "Insert or update a document in a Datastore collection.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "collection": { "type": "string", "description": "Collection name" },
                    "document": { "type": "object", "description": "Document to upsert" },
                    "upsert_key": { "type": "string", "description": "Field to use as upsert key" }
                },
                "required": ["collection", "document"]
            }),
        },
        ToolDefinition {
            name: "datastore_delete".into(),
            description: "Delete documents from a Datastore collection.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "collection": { "type": "string", "description": "Collection name" },
                    "query": { "type": "object", "description": "Filter for documents to delete" }
                },
                "required": ["collection", "query"]
            }),
        },
    ]
}
