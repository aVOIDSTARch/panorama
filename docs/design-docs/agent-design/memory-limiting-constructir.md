import { Memory } from "@mastra/memory"; import { ToolCallFilter, TokenLimiter } from
"@mastra/memory/processors";
const memoryFilteringTools = new Memory({
processors: [
// Example 1: Remove all tool calls/results
new ToolCallFilter(),
// Example 2: Remove only noisy image generation
tool calls/results
new ToolCallFilter({ exclude:
I "generateImageTool"] }),
// Always place TokenLimiter last
new TokenLimiter(127000),
}) ;