```

import { z } from "zod";

const mySchema = z.object({
	definition: z.string(),
	examples:z.array(z.string()),
}) ;

const response = await llm.generate(
	"Define machine learning and give
	examples."
	output: mySchema,
	console.log(response.object);

```