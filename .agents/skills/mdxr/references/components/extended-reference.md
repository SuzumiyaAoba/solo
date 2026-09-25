# Reference components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## CodeGroup

See the example and `mdxr catalog` for the public attributes.

````mdx
<CodeGroup syncKey='language'>

```ts
const value = 1;
```

```python
value = 1
```

</CodeGroup>
````

## PackageInstall

See the example and `mdxr catalog` for the public attributes.

```mdx
<PackageInstall packages="@suzumiyaaoba/mdxr" dev="true" />
```

## ApiExample

See the example and `mdxr catalog` for the public attributes.

```mdx
<ApiExample>

<Request method="GET" path="/items" headers="Accept: application/json" />

<Response status="200" body='{"items":["one"]}' />

</ApiExample>
```

## ApiParams

Request parameters by location, type, required flag, constraints and example.

Data columns: `name,in,type,required,default,constraints,example,description`.

```mdx
<ApiParams data='[{"name":"Example","in":"In","type":"Type","required":"Required","default":"Default","constraints":"Constraints","example":"Example","description":"Review the result before release."}]' />
```

## ObjectSchema

See the example and `mdxr catalog` for the public attributes.

```mdx
<ObjectSchema schema='{"type":"object","required":["id"],"properties":{"id":{"type":"string","description":"Stable identifier"},"tags":{"type":"array","items":{"type":"string"}},"details":{"oneOf":[{"type":"string"},{"type":"number"}]}}}' />
```

## CliReference

Commands, positional arguments, flags, defaults and usage examples.

Data columns: `command,argument,flag,type,default,conflicts,example,description`.

```mdx
<CliReference data='[{"command":"Command","argument":"Argument","flag":"Flag","type":"Type","default":"Default","conflicts":"Conflicts","example":"Example","description":"Review the result before release."}]' />
```

## ConfigReference

Configuration keys, environment and CLI overrides, precedence and defaults.

Data columns: `key,type,default,env,flag,precedence,description`.

```mdx
<ConfigReference data='[{"key":"Key","type":"Type","default":"Default","env":"Env","flag":"Flag","precedence":"Precedence","description":"Review the result before release."}]' />
```

## ErrorCatalog

Error codes, causes, recovery actions and retry guidance.

Data columns: `code,message,cause,action,retry,href`.

```mdx
<ErrorCatalog data='[{"code":"Code","message":"Message","cause":"Cause","action":"Run the verification suite","retry":"Retry","href":"Href"}]' />
```

## CompatibilityMatrix

Platform/runtime/version compatibility with supporting evidence.

Data columns: `platform,runtime,version,status,notes`.

```mdx
<CompatibilityMatrix data='[{"platform":"Platform","runtime":"Runtime","version":"1.2.0","status":"open","notes":"Reviewed with the team."}]' />
```

## DeprecationTimeline

Deprecation and removal milestones linked to replacement and migration.

Data columns: `feature,deprecated,removal,replacement,migration`.

```mdx
<DeprecationTimeline data='[{"feature":"Feature","deprecated":"Deprecated","removal":"Removal","replacement":"Replacement","migration":"Migration"}]' />
```
