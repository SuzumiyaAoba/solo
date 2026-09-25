# Diagrams components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## SequenceDiagram

See the example and `mdxr catalog` for the public attributes.

```mdx
<SequenceDiagram
  nodes='[{"id":"client","label":"Client","lane":"Application"},{"id":"api","label":"API","lane":"Server"},{"id":"db","label":"Database","lane":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## StateDiagram

See the example and `mdxr catalog` for the public attributes.

```mdx
<StateDiagram
  nodes='[{"id":"client","label":"Client","lane":"Application"},{"id":"api","label":"API","lane":"Server"},{"id":"db","label":"Database","lane":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request","guard":"valid","action":"load"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## EntityRelations

See the example and `mdxr catalog` for the public attributes.

```mdx
<EntityRelations data='[{"table":"users","name":"id","type":"uuid"},{"table":"orders","name":"id","type":"uuid"},{"table":"orders","name":"user_id","type":"uuid","fk":"users.id","relation":"many-to-one"}]' />
```

## Swimlane

See the example and `mdxr catalog` for the public attributes.

```mdx
<Swimlane
  nodes='[{"id":"client","label":"Client","lane":"Application"},{"id":"api","label":"API","lane":"Server"},{"id":"db","label":"Database","lane":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## Architecture

See the example and `mdxr catalog` for the public attributes.

```mdx
<Architecture
  nodes='[{"id":"client","label":"Client","lane":"Application","group":"Application"},{"id":"api","label":"API","lane":"Server","group":"Server"},{"id":"db","label":"Database","lane":"Server","group":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## DecisionTree

See the example and `mdxr catalog` for the public attributes.

```mdx
<DecisionTree
  nodes='[{"id":"client","label":"Client","lane":"Application","condition":"Request valid?"},{"id":"api","label":"API","lane":"Server"},{"id":"db","label":"Database","lane":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## MindMap

See the example and `mdxr catalog` for the public attributes.

```mdx
<MindMap
  nodes='[{"id":"client","label":"Client","lane":"Application"},{"id":"api","label":"API","lane":"Server"},{"id":"db","label":"Database","lane":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## JourneyMap

Journey stages, user actions, touchpoints, pain points and improvements.

Data columns: `stage,action,touchpoint,pain,improvement`.

```mdx
<JourneyMap data='[{"stage":"Stage","action":"Run the verification suite","touchpoint":"Touchpoint","pain":"Pain","improvement":"Improvement"}]' />
```

## ServiceTopology

See the example and `mdxr catalog` for the public attributes.

```mdx
<ServiceTopology
  nodes='[{"id":"client","label":"Client","lane":"Application","group":"Application"},{"id":"api","label":"API","lane":"Server","group":"Server"},{"id":"db","label":"Database","lane":"Server","group":"Server"}]'
  edges='[{"from":"client","to":"api","label":"Request"},{"from":"api","to":"db","label":"Query"}]'
/>
```

## DependencyMatrix

See the example and `mdxr catalog` for the public attributes.

```mdx
<DependencyMatrix data='[{"from":"API","to":"DB","value":3},{"from":"Worker","to":"DB","value":1}]' />
```
