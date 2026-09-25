---
type: Research Profile
title: "Amp"
description: "モデルルーティング、共有thread、ローカルとクラウド作業の連続性を学ぶ参照例。"
tags: [research, agent-harness, cloud]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: official-documentation-review
  hands_on: false
  category: cloud
sources:
  - id: amp-docs
    resource: "https://ampcode.com/docs"
    title: "Amp Introduction"
---

# Amp

## 確認できた機能と構造

現在の公式資料では、複数モデルの使い分け、保存・共有できるthread、threadごとに作成するクラウドmachineであるOrb、Web/desktop/mobile/CLIを説明する。CLIにはローカル作業の入口があり、MCP、skills、AGENTS.md、モデルrouting等の資料がある。後方互換性を重視しない製品方針も明記される。[^amp-docs]

## 長所・短所の分析

- **長所:** モデル選択を利用者の仕事から切り離す設計と、端末を閉じても続く作業の体験を比較できる。
- **長所:** threadと実行machineを関連付けることで、複数端末からの継続が理解しやすくなる。
- **短所:** 自動routingでは実行ごとのモデル・費用・結果差を説明できる記録が必要。
- **短所:** 高い更新速度と後方互換性を重視しない方針は、長期固定した組込み用途と相性が悪い可能性がある。
- **短所:** threadの共有と作業環境の共有ではアクセス範囲が違う。別の権限設計が必要。

## Rust + GPUIへの示唆

モデルrouterはコアに埋め込まず、選択理由と実際に使ったprovider/modelをイベントとして残す。クラウドmachineの導入は、ローカルでの再開・監査・exportが完成してから検討する。

関連: [OpenHands](openhands.md)、[費用と継続性](../analysis/reuse.md)。

[^amp-docs]: [Amp Introduction](https://ampcode.com/docs)（参照日: 2026-09-25）。

