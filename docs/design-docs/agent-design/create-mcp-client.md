import { MCClient, MCPServer } from "@mastra/mcp";
// Step 1: Create an MCP client that connects to other MCP servers
const mcp = new MCPClient({
servers: {
weather: {
/ Connect to a remote MCP server via HTTP/SSE url: new URL("http://localhost:1234/sse"),
stocks: {
/ Or connect to a local MCP server via stdio command: "npx" args: ["tsx"
, "stock-server.ts"],
},
},
});
tlmeout: 30000,
// Step 2: Expose all tools from the connected MCP servers via a new
MCPServer
const server = new MCPServer({
name: "Proxy MCP Server"
version: "1.0.0"
tools: await mcp.getTools(), // Aggregate tools from all connected servers
}) ;
// Step 3: Start the proxy MCP server (stdio)
await server.start.Stdio();