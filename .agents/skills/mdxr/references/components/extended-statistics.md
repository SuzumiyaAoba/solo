# Statistics components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## Heatmap

See the example and `mdxr catalog` for the public attributes.

```mdx
<Heatmap data='[{"x":"Mon","y":"API","value":3},{"x":"Tue","y":"API","value":8},{"x":"Mon","y":"Worker","value":5},{"x":"Tue","y":"Worker","value":2}]' />
```

## Histogram

See the example and `mdxr catalog` for the public attributes.

```mdx
<Histogram data='[{"values":[1,1,2,3,3,3,4,5,8,9]}]' options='{"bins":4}' />
```

## BoxPlot

See the example and `mdxr catalog` for the public attributes.

```mdx
<BoxPlot data='[{"name":"A","values":[1,2,3,4,5,20]},{"name":"B","values":[2,3,4,5,6]}]' />
```

## ECDF

See the example and `mdxr catalog` for the public attributes.

```mdx
<ECDF data='[{"values":[1,1,2,3,4,5,8]}]' />
```

## IntervalPlot

See the example and `mdxr catalog` for the public attributes.

```mdx
<IntervalPlot data='[{"name":"A","value":5,"low":3,"high":7},{"name":"B","value":8,"low":6,"high":9}]' />
```

## BulletChart

See the example and `mdxr catalog` for the public attributes.

```mdx
<BulletChart data='[{"name":"Revenue","value":72,"target":85,"ranges":[50,75,100]}]' />
```

## CalendarHeatmap

See the example and `mdxr catalog` for the public attributes.

```mdx
<CalendarHeatmap data='[{"date":"2026-09-01","value":1},{"date":"2026-09-02","value":2},{"date":"2026-09-03","value":3},{"date":"2026-09-04","value":4},{"date":"2026-09-05","value":5},{"date":"2026-09-06","value":6},{"date":"2026-09-07","value":0},{"date":"2026-09-08","value":1},{"date":"2026-09-09","value":2},{"date":"2026-09-10","value":3},{"date":"2026-09-11","value":4},{"date":"2026-09-12","value":5},{"date":"2026-09-13","value":6},{"date":"2026-09-14","value":0},{"date":"2026-09-15","value":1},{"date":"2026-09-16","value":2},{"date":"2026-09-17","value":3},{"date":"2026-09-18","value":4},{"date":"2026-09-19","value":5},{"date":"2026-09-20","value":6},{"date":"2026-09-21","value":0}]' />
```

## ParetoChart

See the example and `mdxr catalog` for the public attributes.

```mdx
<ParetoChart data='[{"name":"Network","value":50},{"name":"Parser","value":30},{"name":"Other","value":20}]' />
```

## CohortTable

See the example and `mdxr catalog` for the public attributes.

```mdx
<CohortTable data='[{"cohort":"July","period":"Week 1","retained":80,"total":100},{"cohort":"July","period":"Week 2","retained":60,"total":100},{"cohort":"August","period":"Week 1","retained":90,"total":100}]' />
```

## ConfusionMatrix

See the example and `mdxr catalog` for the public attributes.

```mdx
<ConfusionMatrix data='[{"actual":"yes","predicted":"yes","count":40},{"actual":"yes","predicted":"no","count":10},{"actual":"no","predicted":"yes","count":5},{"actual":"no","predicted":"no","count":45}]' />
```
