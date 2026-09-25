---
name: mdxr
description: Write rich plan/report documents as MDX using the mdxr component catalog, then render them to standalone HTML with `npx @suzumiyaaoba/mdxr render`. Use when creating plan files, status reports, reviews, or any structured document meant to be viewed as a styled HTML page — and whenever the user explicitly asks for a deliverable written or rendered with mdxr, regardless of document type.
---

# mdxr — agent-authored documents rendered to HTML

Write documents as **Markdown + a small set of JSX components** (MDX). Do NOT write raw HTML: `mdxr render` compiles the document deterministically, so markup, styling and scripts are never emitted by the model.

## Workflow

1. Write the document as `.mdxr/<name>.mdx` at the project root, creating `.mdxr/` if needed (unless the user gave a path). `.mdxr/` is untracked scratch space: if the project is a git repo, make sure `.gitignore` lists `.mdxr/` — append it when missing.
2. Render and open it for the user: `npx @suzumiyaaoba/mdxr render .mdxr/plan.mdx --open` (the default output is the source path with `.html` — `.mdxr/plan.html`; `--open` launches the file in the user's default browser: `open` on macOS, `xdg-open` on Linux, `start` on Windows).
3. Or pipe MDX directly: `cat .mdxr/plan.mdx | npx @suzumiyaaoba/mdxr render > .mdxr/plan.html` (`mdxr render -` also reads stdin; `-o out.html` writes a file — required for `--open`).
4. On errors, the message includes `file:line:col` — fix and re-run. `npx @suzumiyaaoba/mdxr render .mdxr/plan.mdx --format json` prints machine-readable errors.
5. Preview while editing: `npx @suzumiyaaoba/mdxr serve .mdxr/plan.mdx --open` — or pipe: `cat .mdxr/plan.mdx | npx @suzumiyaaoba/mdxr serve --open`
6. Plain-text deliverable: `npx @suzumiyaaoba/mdxr text .mdxr/plan.mdx` renders the document to Markdown readable in a terminal — components become ASCII stand-ins (checkbox lists, bar charts, GFM tables); interactive-only widgets degrade to their text.

## Rules

- **No JS in documents.** `import`/`export` and `{expressions}` are rejected. All attributes are strings: `<Step status="done">`, not `status={...}`.
- Prefer plain Markdown for prose; use components only for structure.
- If a needed component is missing, run `npx @suzumiyaaoba/mdxr catalog --json` to see the full catalog, then define it in the project's component file — see `references/extending.md` for the extension mechanism.
- For component usage, read `references/components.md` (the index — includes a syntax cheatsheet), then only the `references/components/*.md` detail file(s) the document needs.

## Component categories

| Category | Covers | Details |
| --- | --- | --- |
| Document scaffolding | `<Plan>` root, meta row, callouts, TOC, glossary, refs, code blocks | `references/components/document.md` |
| Planning & status | phases, steps, timeline, gantt, decisions, risks, board, matrix, stats | `references/components/planning.md` |
| Code investigation | findings, hypotheses, terminal, traces, searches, files, flows, trees | `references/components/investigation.md` |
| Output artifacts | diff cards, graphs, tests, endpoints, JSON, waterfalls | `references/components/output.md` |
| Reports | code review, CI checks, vuln/dep audits, metrics, schema/env docs, status/release/incident | `references/components/reports.md` |
| Data visualization | bar/line/pie/scatter/radar/funnel/quadrant/bridge/treemap/sankey/venn | `references/components/charts.md` |
| Layout | columns, grid, row, stack, before/after panels | `references/components/layout.md` |
| Reader input | `<Ask>` question forms with copyable Markdown answers | `references/components/forms.md` |
| shadcn/ui | `Button`, `Card`, `Table`, `Tabs`, … — interactive via hydration | `references/components/shadcn.md` |

## Common syntax

| Write | Get |
| --- | --- |
| `:::note` / `:::warning` / `:::decision` / `> [!NOTE]` | `<Callout>` |
| `:::phase{title="…" status="doing"}` | `<Phase>` heading with status badge |
| ` ```ts title="src/x.ts" ` | highlighted code block + filename bar |
| ` ```diff ` / ` ```mermaid ` / ` ```console ` | diff cards / diagram / terminal transcript |
| `` `src/x.ts` `` naming a real file | `<FileRef>` chip with editor link |
| `- [ ]` / `- [x]` | styled task list |
| frontmatter `status:` / `date:` / `owner:` | document header badge + meta row |
