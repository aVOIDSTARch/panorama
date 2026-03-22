--- weatherTool.ts
import { defineTool } from "@mastra/core/tool";
export const weatherTool = defineTool({
name: "weatherTool",
description: "Get the current weather for a city.",
parameters: {
city: { type: "string", description: "City name" },
async execute({ city }) {
/ Dummy implementation
return 'The weather in ${city} is sunny!';
},
});
//
weather-server.ts
import { MCPServer } from "@mastra/mcp"; import { weatherTool } from "./weatherTool";
const server = new MCPServer({
name: "Weather Server", version: "1.0.0", tools: {weatherTool },
}) ;
await server. startStdio();
//
agent.ts
import { MCPClient } from "@mastra/mcp"; import { Agent } from "@mastra/core/agent"; import { openai } from "@ai-sdk/openai";
const mcp = new MCPClient {
servers: {
weather: 1
command: "npx",
args: ["tsx", "weather-server. ts"],
},
},
timeout: 30000,
}) ;
const agent = new Agent ({
name: "Weather Agent",
instructions: "You can answer weather questions using the weather tool.", model: openai("gpt-4"),
tools: await mcp.getTools(),
}) ;