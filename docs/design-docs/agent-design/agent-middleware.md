• • •
const supportAgent = new Agent ( {
name: "Dynamic Support Agent",
instructions: async ({ runtimeContext }) => {
const userTier = runtimeContext.get "user-tier");
const language = runtimeContext.get("language") "language");
return 'You are a customer support agent for our Saas platform.
The current user is on the ${userTier} tier and prefers ${language}
language.
For ${userTier} tier users:
${userTier === "free" ? "- Provide basic support and documentation
links" : ""}
${userTier === "pro" ? "- Offer detailed technical support and best
practices" : ""}
${userTier === "enterprise" ? "- Provide priority support with
custom solutions" : ""}
Always respond in ${language} language.';
},
model: ({runtimeContext }) => {
const userTier = runtimeContext.get "user-tier");
return userTier === "enterprise"
? openai("gpt-4")
: openai("gpt-3.5-turbo") ;
},
tools: ({ runtimeContext }) => {
const userTier = runtimeContext.get "user-tier");
const baseTools = [knowledgeBase, ticketSystem];
if (userTier === "pro" || userTier === "enterprise") {
baseTools.push(advancedAnalytics);
if (userTier === "enterprise") {
baseTools.push(customIntegration);
}
return baseTools;
}) ;