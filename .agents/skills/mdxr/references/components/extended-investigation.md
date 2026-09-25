# Investigation components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## Reproduction

Bug reproduction: environment, prerequisite, action, expected and actual results.

Data columns: `step,environment,prerequisite,action,expected,actual`.

```mdx
<Reproduction data='[{"step":"Step","environment":"staging","prerequisite":"Prerequisite","action":"Run the verification suite","expected":"Ready","actual":"Ready"}]' />
```

## Logs

Searchable timestamped log entries by level and source.

Data columns: `time,level,source,message`.

```mdx
<Logs data='[{"time":"2026-09-01T09:00:00Z","level":"Level","source":"Source","message":"Message"}]' />
```

## JsonDiff

See the example and `mdxr catalog` for the public attributes.

```mdx
<JsonDiff
  before='{"type":"string","description":"Identifier"}'
  after='{"type":"number","description":"Identifier"}'
/>
```

## ConfigDiff

See the example and `mdxr catalog` for the public attributes.

```mdx
<ConfigDiff
  before='[{"name":"defaults","values":{"retries":2}}]'
  after='[{"name":"defaults","values":{"retries":2}},{"name":"production","values":{"retries":4}}]'
/>
```

## ApiDiff

See the example and `mdxr catalog` for the public attributes.

```mdx
<ApiDiff
  before='{"type":"string","description":"Identifier"}'
  after='{"type":"number","description":"Identifier"}'
/>
```

## SchemaDiff

See the example and `mdxr catalog` for the public attributes.

```mdx
<SchemaDiff
  before='{"type":"string","description":"Identifier"}'
  after='{"type":"number","description":"Identifier"}'
/>
```

## ImpactMap

See the example and `mdxr catalog` for the public attributes.

```mdx
<ImpactMap
  nodes='[{"id":"client","label":"Client","lane":"Application"},{"id":"api","label":"API","lane":"Server"},{"id":"db","label":"Database","lane":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
  options='{"changed":["api"]}'
/>
```

## DataLineage

See the example and `mdxr catalog` for the public attributes.

```mdx
<DataLineage
  nodes='[{"id":"client","label":"Client","lane":"Application"},{"id":"api","label":"API","lane":"Server"},{"id":"db","label":"Database","lane":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## SymbolOutline

Symbol paths, kinds, signatures, parents and source locations.

Data columns: `name,kind,parent,signature,file,lines`.

```mdx
<SymbolOutline data='[{"name":"Example","kind":"Kind","parent":"Parent","signature":"Signature","file":"File","lines":"Lines"}]' />
```

## CodeWalkthrough

See the example and `mdxr catalog` for the public attributes.

```mdx
<CodeWalkthrough data='[{"title":"Parse","code":"const value = 2;\nreturn value * 3;","lines":"1","explanation":"Read the input."},{"title":"Compute","code":"const value = 2;\nreturn value * 3;","lines":"2","explanation":"Return three times the input."}]' />
```
