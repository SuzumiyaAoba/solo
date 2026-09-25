# Operations components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## Runbook

Operational steps with prerequisites, commands, expected results, stop and recovery conditions.

Data columns: `step,prerequisite,command,expected,stop,recovery`.

```mdx
<Runbook data='[{"step":"Step","prerequisite":"Prerequisite","command":"Command","expected":"Ready","stop":"Stop","recovery":"Recovery"}]' />
```

## Rollout

Rollout stages, exposure, metrics, promotion gates and rollback conditions.

Data columns: `stage,target,percent,metric,gate,rollback,status`.

```mdx
<Rollout data='[{"stage":"Stage","target":"Target","percent":"Percent","metric":"Metric","gate":"Gate","rollback":"Rollback","status":"open"}]' />
```

## SLO

See the example and `mdxr catalog` for the public attributes.

```mdx
<SLO data='[{"name":"API availability","target":99.9,"total":100000,"bad":40}]' />

<ErrorBudget data='[{"name":"API availability","target":99.9,"total":100000,"bad":40}]' />
```

## AlertRules

Alert thresholds, evaluation duration, owner and linked response procedure.

Data columns: `name,metric,condition,duration,owner,runbook`.

```mdx
<AlertRules data='[{"name":"Example","metric":"Metric","condition":"Condition","duration":"Duration","owner":"Platform","runbook":"Runbook"}]' />
```

## FeatureFlags

Feature flag state, targeting, ownership and retirement across environments.

Data columns: `flag,environment,enabled,condition,owner,retire`.

```mdx
<FeatureFlags data='[{"flag":"Flag","environment":"staging","enabled":"Enabled","condition":"Condition","owner":"Platform","retire":"Retire"}]' />
```

## RecoveryPlan

Recovery order, dependencies, RPO/RTO, backups and verification.

Data columns: `step,depends,service,rpo,rto,backup,action,verification`.

```mdx
<RecoveryPlan data='[{"step":"Step","depends":"Depends","service":"Service","rpo":"Rpo","rto":"Rto","backup":"Backup","action":"Run the verification suite","verification":"Verification"}]' />
```

## PermissionMatrix

Role/resource/action permissions and conditions.

Data columns: `role,resource,action,allowed,condition`.

```mdx
<PermissionMatrix data='[{"role":"Role","resource":"Resource","action":"Run the verification suite","allowed":"Allowed","condition":"Condition"}]' />
```

## ThreatModel

Assets, threats, attack paths, controls and residual risks.

Data columns: `asset,threat,path,control,residual,owner`.

```mdx
<ThreatModel data='[{"asset":"Asset","threat":"Threat","path":"Path","control":"Control","residual":"Residual","owner":"Platform"}]' />
```

## Remediation

Vulnerability remediation with owner, due date, fix and verification evidence.

Data columns: `vulnerability,severity,owner,due,fix,verification,status`.

```mdx
<Remediation data='[{"vulnerability":"Vulnerability","severity":"Severity","owner":"Platform","due":"Due","fix":"Fix","verification":"Verification","status":"open"}]' />
```

## MaintenanceWindow

Maintenance windows with affected services, explicit time zones and duration.

Data columns: `service,start,end,durationMinutes,impact,recovery`.

```mdx
<MaintenanceWindow data='[{"name":"Database upgrade","start":"2026-09-01T02:00:00Z","end":"2026-09-01T02:30:00Z","owner":"Platform","status":"planned"}]' />
```
