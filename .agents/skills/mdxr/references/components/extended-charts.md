# Charts components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## ViolinPlot

See the example and `mdxr catalog` for the public attributes.

```mdx
<ViolinPlot data='[{"name":"A","values":[1,2,2,3,4,5,6]},{"name":"B","values":[3,4,4,5,6,8]}]' />
```

## RidgelinePlot

See the example and `mdxr catalog` for the public attributes.

```mdx
<RidgelinePlot data='[{"name":"A","values":[1,2,2,3,4,5,6]},{"name":"B","values":[3,4,4,5,6,8]}]' />
```

## DotPlot

See the example and `mdxr catalog` for the public attributes.

```mdx
<DotPlot data='[{"name":"A","value":12},{"name":"B","value":8}]' />
```

## SlopeChart

See the example and `mdxr catalog` for the public attributes.

```mdx
<SlopeChart data='[{"name":"A","before":5,"after":9},{"name":"B","before":8,"after":6}]' />
```

## BumpChart

See the example and `mdxr catalog` for the public attributes.

```mdx
<BumpChart
  data='[{"name":"A","values":[1,2,1]},{"name":"B","values":[2,1,2]}]'
  options='{"labels":["Q1","Q2","Q3"]}'
/>
```

## DumbbellChart

See the example and `mdxr catalog` for the public attributes.

```mdx
<DumbbellChart data='[{"name":"A","before":5,"after":9},{"name":"B","before":8,"after":6}]' />
```

## Sunburst

See the example and `mdxr catalog` for the public attributes.

```mdx
<Sunburst data='[{"id":"app","name":"App"},{"id":"ui","parent":"app","name":"UI","value":60},{"id":"core","parent":"app","name":"Core","value":40}]' />
```

## ChordDiagram

See the example and `mdxr catalog` for the public attributes.

```mdx
<ChordDiagram data='[{"from":"API","to":"DB","value":30},{"from":"Worker","to":"DB","value":20},{"from":"API","to":"Worker","value":10}]' />
```

## UpSetPlot

See the example and `mdxr catalog` for the public attributes.

```mdx
<UpSetPlot data='[{"sets":["A"],"value":10},{"sets":["B"],"value":7},{"sets":["A","B"],"value":4}]' />
```

## SmallMultiples

See the example and `mdxr catalog` for the public attributes.

```mdx
<SmallMultiples>

<DotPlot data='[{"name":"A","value":12}]' title="January" />

<DotPlot data='[{"name":"A","value":25}]' title="February" />

</SmallMultiples>
```
