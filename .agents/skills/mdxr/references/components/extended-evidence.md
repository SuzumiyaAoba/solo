# Evidence components

All data and options attributes contain literal JSON strings. No MDX expressions are needed. HTML includes interactive controls; ASCII retains static data.

## Sources

See the example and `mdxr catalog` for the public attributes.

```mdx
A documented claim <Cite source="manual" />.

<Sources>

<Source
  id="manual"
  href="https://example.com/manual"
  title="Example manual"
  author="Example team"
  published="2026-09-01"
/>

</Sources>
```

## CrossRef

See the example and `mdxr catalog` for the public attributes.

```mdx
See <CrossRef target="reference-figure" />.

<Figure src='assets/before.svg' id='reference-figure' caption='Document rendering'>

An example figure.

</Figure>
```

## Evidence

Evidence with source, revision, capture time and environment.

Data columns: `claim,source,captured,commit,environment,result`.

```mdx
<Evidence data='[{"claim":"Claim","source":"Source","captured":"Captured","commit":"Commit","environment":"staging","result":"pass"}]' />
```

## Provenance

Reproducible report inputs, tools, versions and settings.

Data columns: `input,tool,version,config,hash,generated`.

```mdx
<Provenance data='[{"input":"Input","tool":"Tool","version":"1.2.0","config":"Config","hash":"Hash","generated":"Generated"}]' />
```

## Assumptions

Assumptions with verification state, owner and review trigger.

Data columns: `assumption,status,owner,evidence,revisit`.

```mdx
<Assumptions data='[{"assumption":"Assumption","status":"open","owner":"Platform","evidence":"tests/release.test.ts","revisit":"Revisit"}]' />
```

## Limitations

Scope and measurement limitations with their consequences.

Data columns: `limitation,scope,consequence,followup`.

```mdx
<Limitations data='[{"limitation":"Limitation","scope":"Renderer","consequence":"Consequence","followup":"Followup"}]' />
```

## Sidenote

See the example and `mdxr catalog` for the public attributes.

```mdx
<Sidenote title='Context'>

This note stays close to the discussion.

</Sidenote>
```

## TermRef

See the example and `mdxr catalog` for the public attributes.

```mdx
<Glossary>
  <Term name="cache">Stored results reused for later requests.</Term>
</Glossary>

Use a <TermRef term="cache" />.
```

## Include

See the example and `mdxr catalog` for the public attributes.

```mdx
<Include
  path="../../tests/fixtures/extended-include.mdx"
  section="shared-context"
/>
```

## DocumentHistory

Document revisions, authors, reasons and superseding versions.

Data columns: `version,date,author,reason,supersededBy`.

```mdx
<DocumentHistory data='[{"version":"1.2.0","date":"2026-09-01","author":"Author","reason":"Required for release","supersededBy":"Supersededby"}]' />
```
