---
type: Research Profile
title: "CrewAI"
description: "決定的なFlowと役割を持つagent群を分けるオーケストレーションの参照例。"
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
  - id: crewai-intro
    resource: "https://docs.crewai.com/v1.15.22/en/introduction"
    title: "CrewAI Introduction"
---

# CrewAI

## 確認できた機能と構造

Flowはstate、event、条件分岐、loopを管理し、その一部を役割・目標・toolを持つCrewへ委任する。現行資料は本番用途ではFlowから始める方針を説明する。[^crewai-intro]

## 長所・短所の分析

- **長所:** 決まった作業順序と、自律的な問題解決を担当する範囲を分けられる。
- **長所:** agentの役割と終了条件を定義する練習として分かりやすい。
- **短所:** 役割を増やすだけで専門性や正しさが増すとは限らず、引継ぎのトークンと遅延が増える。
- **短所:** 共有filesystemへ複数agentが書く場合、workflowの状態管理とは別に競合制御が必要。
- **短所:** desktopのPTY、diff、IME、低遅延描画は別の実装領域である。

## Rust + GPUIへの示唆

将来、定期的なrepository点検やレビューpipelineを追加する際の参考にする。初期は「一つのtaskを一つのagentが実行し、人がレビュー」の構成で、計測結果に応じて分業を追加する。

関連: [LangGraph・Deep Agents](langgraph-deepagents.md)、[ロードマップ](../analysis/roadmap.md)。

[^crewai-intro]: [CrewAI Introduction](https://docs.crewai.com/v1.15.22/en/introduction)（参照日: 2026-09-25）。

