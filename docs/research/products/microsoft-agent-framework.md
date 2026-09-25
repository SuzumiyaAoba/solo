---
type: Research Profile
title: "Microsoft Agent Framework・AutoGen・Semantic Kernel"
description: "agent抽象、harness、workflowを統合する現行基盤と旧系譜の整理。"
tags: [research, agent-harness, framework]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: official-documentation-review
  hands_on: false
  category: framework
sources:
  - id: microsoft-agent
    resource: "https://learn.microsoft.com/en-us/agent-framework/overview/"
    title: "Microsoft Agent Framework overview"
---

# Microsoft Agent Framework

## 確認できた機能と構造

現行資料はAgent、Harness Agent、Workflow、Integrationを区別する。Harness Agentにはplanning、context compaction、file/memory、承認、観測の組込み能力がある。session、context provider、middleware、MCPも扱う。資料はAutoGenのagent抽象とSemantic Kernelの企業向け機能を合わせる系譜を説明する。[^microsoft-agent]

## 長所・短所の分析

- **長所:** agentを動かす薄い部品と、機能を揃えたharnessを同じ概念だと混同しない整理が参考になる。
- **長所:** 通常の関数とagentをworkflowで組み合わせ、企業の既存サービスへ接続する設計に向く。
- **短所:** 開発対象が広く、軽いローカルアプリには不要な抽象や依存が含まれる可能性がある。
- **短所:** 旧AutoGen/Semantic Kernelのサンプルを、新frameworkのAPIや保守方針へそのまま当てはめられない。
- **未確認:** SDKごとの成熟度、Goのpreview等に差がある。旧製品すべてが終了したという確認はしていない。

## Rust + GPUIへの示唆

中核の小さな `Agent` と、permission、context、traceを揃えた `Harness` を分ける。workflowのためにgraph言語を先に作るより、通常のRust関数で表現できる工程を優先する。

AutoGenのmulti-agent会話は歴史的な比較として残すが、最初の製品を複数agent前提にはしない。複雑性を追加する時は単一agentとの差を評価する。

関連: [CrewAI](crewai.md)、[ロードマップ](../analysis/roadmap.md)。

[^microsoft-agent]: [Microsoft Agent Framework overview](https://learn.microsoft.com/en-us/agent-framework/overview/)（参照日: 2026-09-25）。

