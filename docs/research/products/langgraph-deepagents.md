---
type: Research Profile
title: "LangGraph・Deep Agents"
description: "永続実行runtimeと高機能harnessを層として分ける参照例。"
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
  - id: langgraph
    resource: "https://docs.langchain.com/oss/python/langgraph/overview"
    title: "LangGraph overview"
  - id: deepagents
    resource: "https://docs.langchain.com/oss/python/deepagents/overview"
    title: "Deep Agents overview"
---

# LangGraph・Deep Agents

## 確認できた機能と構造

LangGraphは状態を持つ長時間workflowのruntimeで、durable execution、streaming、human-in-the-loop、persistenceを扱う。プロンプトやagent構造を一つに固定するものではない。[^langgraph]

Deep Agentsはその上のharnessとして、ファイル操作、文脈の要約・大きな結果の退避、memory、subagent、必要に応じたplanning/skillsを提供する。toolの人による確認にはLangGraphのinterruptを使う。仮想filesystemと実際のsandboxを区別している。[^deepagents]

## 長所・短所の分析

- **長所:** 実行状態の管理と、モデルにどんな道具を渡すかの設計を分離できる。
- **長所:** 複雑な分岐、承認待ち、再開を明示的な状態として扱いやすい。
- **短所:** graphやmiddlewareの層が増えるほど、単純なコーディングloopでも動作を理解する負担が増す。
- **短所:** checkpointした状態を再実行すれば、外部副作用が必ず一度だけ発生するという保証にはならない。
- **短所:** 仮想filesystemはcontext管理の仕組みでもあり、それ自体をOS sandboxと見なせない。

## Rust + GPUIへの示唆

そのまま採用するなら別サービス、Rustで独自実装するなら状態機械とinterruptの設計を学ぶ。最初から一般的なworkflow engineを作らず、単一turnの状態遷移と永続eventを完成させる。

大きなtool結果をartifactへ退避し、モデルには概要と参照先を渡す方式を取り入れる。圧縮された文脈と完全な監査記録は別に保存する。

関連: [コンテキスト設計](../analysis/context-memory.md)、[復旧設計](../analysis/security-recovery.md)。

[^langgraph]: [LangGraph overview](https://docs.langchain.com/oss/python/langgraph/overview)（参照日: 2026-09-25）。
[^deepagents]: [Deep Agents overview](https://docs.langchain.com/oss/python/deepagents/overview)（参照日: 2026-09-25）。

