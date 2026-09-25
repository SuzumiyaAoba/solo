# Planning components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## Traceability

Requirement → tasks → files → tests coverage with missing links.

Data columns: `id,requirement,tasks,files,tests,missing,status`.

```mdx
<Traceability data='[{"id":"REQ-1","requirement":"Offline export","tasks":["T-1"],"files":["src/render.ts"],"tests":["export.test.ts"]},{"id":"REQ-2","requirement":"Keyboard support","tasks":["T-2"],"files":["src/ui/data-table.tsx"],"tests":[]}]' />
```

## DecisionMatrix

Weighted criteria comparison; options.weights maps criteria to nonnegative weights.

Data columns: `name,score`.

```mdx
<DecisionMatrix
  data='[{"name":"Simple","cost":9,"reliability":6},{"name":"Resilient","cost":5,"reliability":9}]'
  options='{"weights":{"cost":0.4,"reliability":0.6}}'
/>
```

## RiskRegister

Risks ordered by likelihood × impact, with owners and mitigation.

Data columns: `name,likelihood,impact,exposure,owner,mitigation,status`.

```mdx
<RiskRegister data='[{"id":"R1","risk":"Migration delay","likelihood":3,"impact":4,"owner":"Platform","mitigation":"Staged rollout","status":"open"}]' />
```

## Scope

Included, excluded and deferred scope with rationale.

Data columns: `item,scope,reason,phase`.

```mdx
<Scope data='[{"item":"Item","scope":"Renderer","reason":"Required for release","phase":"Phase"}]' />
```

## ActionItems

Actions with owner, deadline, acceptance criteria and completion evidence.

Data columns: `action,owner,due,acceptance,evidence,status`.

```mdx
<ActionItems data='[{"action":"Run the verification suite","owner":"Platform","due":"Due","acceptance":"Acceptance","evidence":"tests/release.test.ts","status":"open"}]' />
```

## Estimate

Three-point PERT estimates and standard deviations.

Data columns: `name,optimistic,likely,pessimistic,expected,deviation,unit`.

```mdx
<Estimate data='[{"name":"Parser","optimistic":2,"likely":4,"pessimistic":8,"unit":"days"}]' />
```

## DependencyPlan

Task dependencies, earliest schedule, slack and critical path; rejects cycles.

Data columns: `id,depends,duration,start,end,slack,critical`.

```mdx
<DependencyPlan data='[{"id":"design","duration":2},{"id":"build","duration":5,"depends":["design"]},{"id":"docs","duration":2,"depends":["design"]},{"id":"release","duration":1,"depends":["build","docs"]}]' />
```

## RACI

Responsibility assignment: responsible, accountable, consulted, informed.

Data columns: `task,responsible,accountable,consulted,informed`.

```mdx
<RACI data='[{"task":"Task","responsible":"Responsible","accountable":"Accountable","consulted":"Consulted","informed":"Informed"}]' />
```

## Objectives

Objectives linked to measurable key results and owners.

Data columns: `objective,keyResults,owner,due,status`.

```mdx
<Objectives data='[{"objective":"Objective","keyResults":"Keyresults","owner":"Platform","due":"Due","status":"open"}]' />

<KeyResults data='[{"name":"Pass rate","baseline":80,"current":92,"target":99}]' />
```

## CapacityPlan

Capacity, allocation, remaining effort and overload by person or team.

Data columns: `owner,period,capacity,allocated,remaining,utilization,status`.

```mdx
<CapacityPlan data='[{"name":"Platform","capacity":40,"allocated":32},{"name":"Design","capacity":20,"allocated":24}]' />
```
