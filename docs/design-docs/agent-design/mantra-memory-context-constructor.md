/ Example: User asks about a past feature discussion
await agent. stream 'What did we decide about the search feature last week?', ‹ memoryoptions: {
LastMessages: 10,
semanticRecall: {
topK: 3,
messageRange: 2,
},
5,
}) ;