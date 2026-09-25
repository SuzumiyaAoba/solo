# Evaluation components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## EvalReport

Evaluation cases with inputs, expected/actual output, rubric and verdict.

Data columns: `name,input,expected,actual,rubric,score,status`.

```mdx
<EvalReport data='[{"id":"E1","input":"2+2","expected":"4","actual":"4"},{"id":"E2","input":"3+3","expected":"6","actual":"7"}]' />

<EvalCase data='[{"id":"E1","input":"2+2","expected":"4","actual":"4"}]' />
```

## ModelComparison

Model results under a shared task, with quality, latency, cost and failures.

Data columns: `model,task,quality,latency,cost,failures,notes`.

```mdx
<ModelComparison data='[{"model":"Model","task":"Task","quality":"Quality","latency":"Latency","cost":"Cost","failures":"Failures","notes":"Reviewed with the team."}]' />
```

## Conversation

See the example and `mdxr catalog` for the public attributes.

```mdx
<Conversation data='[{"role":"user","content":"Summarize the build.","time":"10:00"},{"role":"assistant","content":"All checks passed.","time":"10:01"}]' />
```

## ToolCall

Tool calls including arguments, result, elapsed time and outcome.

Data columns: `tool,arguments,result,duration,status`.

```mdx
<ToolCall data='[{"tool":"Tool","arguments":"Arguments","result":"pass","duration":"Duration","status":"open"}]' />
```

## TokenUsage

See the example and `mdxr catalog` for the public attributes.

```mdx
<TokenUsage data='[{"name":"Example run","input":10000,"cached":6000,"output":800,"inputRate":2,"cacheRate":0.2,"outputRate":8}]' />
```

## PromptTemplate

See the example and `mdxr catalog` for the public attributes.

```mdx
<PromptTemplate
  data='[{"name":"topic","label":"Topic","value":"rendering","description":"Subject of the summary."}]'
  template="Summarize {{topic}} in three sentences."
/>
```

## DatasetProfile

Per-column types, missingness, uniqueness and numeric summaries.

Data columns: `column,rows,missing,missingRate,unique,types,min,max,mean`.

```mdx
<DatasetProfile data='[{"id":"a","age":21,"team":"A"},{"id":"b","age":null,"team":"B"},{"id":"c","age":35,"team":"A"}]' />
```

## DataValidation

Data validation by field rules (required/type/min/max/enum) in options.rules.

Data columns: `field,rule,violations,rows,status`.

```mdx
<DataValidation
  data='[{"id":"a","age":21},{"id":"b","age":-1}]'
  options='{"rules":[{"field":"age","type":"number","min":0,"required":true}]}'
/>
```

## Experiment

Binomial experiment results with Wilson 95% intervals; does not infer a winner.

Data columns: `variant,count,successes,rate,lower95,upper95,condition`.

```mdx
<Experiment data='[{"name":"Control","count":1000,"successes":80},{"name":"Variant","count":1000,"successes":105}]' />
```

## DatasetDiff

See the example and `mdxr catalog` for the public attributes.

```mdx
<DatasetDiff
  before='[{"id":"one","value":2},{"id":"old","value":3}]'
  after='[{"id":"new","value":4},{"id":"one","value":5}]'
/>
```
