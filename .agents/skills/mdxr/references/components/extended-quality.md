# Quality components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## TestMatrix

Test combinations with missing runs; options.axes enumerates the expected axes.

Data columns: `name,environment,version,status,duration`.

```mdx
<TestMatrix
  data='[{"os":"linux","runtime":"22","status":"pass"}]'
  options='{"axes":{"os":["linux","mac"],"runtime":["22","24"]}}'
/>
```

## Coverage

Covered/total observations, coverage rate and uncovered count.

Data columns: `file,kind,covered,total,coverage,uncovered,lines`.

```mdx
<Coverage data='[{"name":"Parser","covered":88,"total":100}]' />
```

## BenchmarkSuite

See the example and `mdxr catalog` for the public attributes.

```mdx
<BenchmarkSuite data='[{"name":"Render","samples":[9,10,12,8,11],"unit":"ms"}]' />
```

## TestHistory

Run history, pass rate and flaky classification.

Data columns: `name,runs,attempts,passRate,flaky,retries`.

```mdx
<TestHistory data='[{"name":"Compile","runs":["pass","pass","pass"]},{"name":"Network","runs":["pass","fail","pass"]}]' />
```

## VisualDiff

See the example and `mdxr catalog` for the public attributes.

```mdx
<VisualDiff
  expected="assets/before.svg"
  actual="assets/after.svg"
  diff="assets/diff.svg"
  threshold="0.01"
  mismatch="0.02"
/>
```

## AccessibilityReport

Accessibility findings with rule, affected element, severity and fix.

Data columns: `rule,element,severity,description,fix,status`.

```mdx
<AccessibilityReport data='[{"rule":"Rule","element":"Element","severity":"Severity","description":"Review the result before release.","fix":"Fix","status":"open"}]' />
```

## BundleReport

See the example and `mdxr catalog` for the public attributes.

```mdx
<BundleReport data='[{"id":"app","name":"App"},{"id":"ui","parent":"app","value":60000,"before":70000,"gzip":18000,"reason":"Document controls"},{"id":"core","parent":"app","value":40000,"before":35000,"gzip":12000,"reason":"Compiler"}]' />
```

## Flamegraph

See the example and `mdxr catalog` for the public attributes.

```mdx
<Flamegraph data='[{"id":"render","value":100},{"id":"parse","parent":"render","value":25},{"id":"compile","parent":"render","value":60},{"id":"highlight","parent":"compile","value":20}]' />
```

## QueryPlan

See the example and `mdxr catalog` for the public attributes.

```mdx
<QueryPlan data='[{"id":"scan","operation":"Index scan","estimatedRows":100,"actualRows":120,"time":5},{"id":"join","parent":"scan","operation":"Nested loop","estimatedRows":10,"actualRows":100,"time":12}]' />
```

## ContractResults

Provider/consumer contract expectations and actual test outcomes.

Data columns: `provider,consumer,contract,expected,actual,status`.

```mdx
<ContractResults data='[{"provider":"Provider","consumer":"Consumer","contract":"Contract","expected":"Ready","actual":"Ready","status":"open"}]' />
```
