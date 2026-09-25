---
type: Research Profile
title: "Letta"
description: "永続メモリーと実行ハーネスの世代差、長期利用の設計を学ぶ参照例。"
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
  - id: letta-sdk
    resource: "https://docs.letta.com/v1-sdk"
    title: "Letta V1 SDK and V2 comparison"
---

# Letta

## 確認できた機能と構造

公式資料はV1 SDKとV2 Agent SDKを区別する。V1はstateful agentとmemory blocks、V2はlocalでのtool実行、永続filesystem、MemFS/git-tracked memory等を扱う。確認した比較表ではV2 Agent SDKはTypeScript向けで、旧MemGPT概念ページからもSDK案内へredirectした。[^letta-sdk]

## 長所・短所の分析

- **長所:** 単発のchatを超え、session間で情報を持ち越す設計を検討できる。
- **長所:** memoryを編集・版管理できる資産として扱う発想は、OKFのような外部知識との関係を整理しやすい。
- **短所:** 過去の誤判断や古い情報を長期保存すると、同じ誤りを繰り返す。書込み・検証・失効の規則が必要。
- **短所:** 利用者、project、repositoryを跨いだmemory混在は、関連性と情報の境界を壊し得る。
- **短所:** SDK世代を取り違えると、tool実行場所や対応言語の前提が変わる。

## Rust + GPUIへの示唆

memoryは会話履歴の別名にしない。保存候補、出典、適用scope、確認日、失効条件を持ち、利用者が見て消せるようにする。

初期版は自動学習memoryより、明示されたproject指示と検証済みの作業メモを優先する。メモを読むたびにsystem権限へ昇格させない。

関連: [コンテキスト設計](../analysis/context-memory.md)、[OKF形式と調査方法](../methodology.md)。

[^letta-sdk]: [Letta V1 SDK and V2 comparison](https://docs.letta.com/v1-sdk)（参照日: 2026-09-25）。

