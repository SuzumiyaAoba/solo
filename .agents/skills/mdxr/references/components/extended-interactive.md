# Interactive components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## DataTable

See the example and `mdxr catalog` for the public attributes.

```mdx
<DataTable data='[{"id":"alpha","name":"Alpha","value":20,"status":"ready"},{"id":"beta","name":"Beta","value":10,"status":"pending"},{"id":"gamma","name":"Gamma","value":30,"status":"ready"}]' />
```

## FilterPanel

See the example and `mdxr catalog` for the public attributes.

```mdx
<FilterPanel fields='status'>

<DataTable data='[{"id":"alpha","name":"Alpha","value":20,"status":"ready"},{"id":"beta","name":"Beta","value":10,"status":"pending"},{"id":"gamma","name":"Gamma","value":30,"status":"ready"}]' />

<DotPlot data='[{"name":"Alpha","value":20,"status":"ready"},{"name":"Beta","value":10,"status":"pending"}]' />

</FilterPanel>
```

## Checklist

See the example and `mdxr catalog` for the public attributes.

```mdx
<Checklist data='[{"id":"tests","label":"Run tests","checked":true,"owner":"Platform"},{"id":"review","label":"Review changes","checked":false}]' />
```

## Ranking

See the example and `mdxr catalog` for the public attributes.

```mdx
<Ranking data='[{"id":"speed","label":"Performance"},{"id":"clarity","label":"Clarity"},{"id":"cost","label":"Cost"}]' />
```

## Calculator

See the example and `mdxr catalog` for the public attributes.

```mdx
<Calculator
  data='[{"name":"users","label":"Users","value":100,"min":0},{"name":"price","label":"Price","value":12,"min":0}]'
  operation="product"
  unit="USD"
/>
```

## Wizard

See the example and `mdxr catalog` for the public attributes.

```mdx
<Wizard data='[{"name":"target","label":"Target","options":["Web","CLI"],"required":true},{"name":"url","label":"Site URL","when":{"target":"Web"},"required":true},{"name":"notes","label":"Notes","type":"textarea"}]' />
```

## SyncedTabs

See the example and `mdxr catalog` for the public attributes.

```mdx
<SyncedTabs syncKey='platform'>

<TabItem label='Linux'>

Linux instructions 1

</TabItem>

<TabItem label='macOS'>

macOS instructions 1

</TabItem>

</SyncedTabs>

<SyncedTabs syncKey='platform'>

<TabItem label='Linux'>

Linux instructions 2

</TabItem>

<TabItem label='macOS'>

macOS instructions 2

</TabItem>

</SyncedTabs>
```

## DocumentSearch

See the example and `mdxr catalog` for the public attributes.

```mdx
<DocumentSearch />

### Searchable section

The search finds rendering instructions in this document.
```

## AnswerSheet

See the example and `mdxr catalog` for the public attributes.

```mdx
<AnswerSheet>

<Ask title="Project">
  <Question name="project" type="text" label="Project name" />
  <Question name="ready" type="toggle" label="Ready" />
</Ask>

</AnswerSheet>
```

## DownloadData

See the example and `mdxr catalog` for the public attributes.

```mdx
<DownloadData
  data='[{"id":"alpha","name":"Alpha","value":20,"status":"ready"},{"id":"beta","name":"Beta","value":10,"status":"pending"},{"id":"gamma","name":"Gamma","value":30,"status":"ready"}]'
  filename="example"
/>
```
