/ Create a step with defined input/output schemas and execution logic
const inputSchema = z.object({
inputValue: z.string(),
}) ;
const myStep = createStep({
id: "my-step" description:
"Does something useful",
inputSchema,
outputSchema: z.object({
outputValue: z.string(),
/ Optional: Define the resume schema for step resumption
resumeSchema: z.object({
resumeValue : z.string(),
// Optional: Define the suspend schema for step suspension
suspendSchema: z.object({
suspendValue: z.string(),
execute: async ({
inputData, mastra, getStepResult, getInitData,
runtimeContext,
const otherStepOutput = getStepResult(step2);
const initData = getInitData<typeof inputSchema>(); // typed as the input schema
variable (zod schema)
return 1
outputValue: 'Processed: ${inputData. inputValue}, $finitData.startValue}
(runtimeContextValue: $fruntimeContext.get("runtimeContextValue")});
} ;
｝
})