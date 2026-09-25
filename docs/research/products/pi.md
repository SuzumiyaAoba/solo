---
type: Research Profile
title: "Pi Coding Agent"
description: "最小のコア、ツリー状セッション、RPC、拡張による機能追加の参照例。"
tags: [research, agent-harness, cli]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: official-documentation-review
  hands_on: false
  category: cli
sources:
  - id: pi-readme
    resource: "https://github.com/earendil-works/pi/tree/main/packages/coding-agent"
    title: "Pi Coding Agent README"
---

# Pi Coding Agent

## 確認できた機能と構造

旧badlogic/pi-monoの参照先は調査時点でearendil-works/piへ移る。Piは対話、JSONL、RPCの実行形式を持ち、extensions、skills、prompt templatesで拡張する。JSONLセッションの各項目にid/parentIdを持たせ、会話の木を保存する。分岐、再開、圧縮、使用量表示、実行途中のsteeringを扱う。[^pi-readme]

公式の設計方針ではMCP、サブエージェント、権限確認popup、plan mode等を標準コアに詰め込まず、必要なら拡張や外部環境を使う。プロジェクトファイルを信頼するかの確認と、各ツール実行の承認は別である。[^pi-readme]

## 長所・短所の分析

- **長所:** コアの責任が比較的小さく、機能を足す前の最小実行ループを理解しやすい。
- **長所:** セッションを木として保存する考え方は、試行比較、分岐の再利用、履歴の損失防止に向く。
- **短所:** 欲しい機能を拡張で組み合わせるほど、組合せ検証と保守は利用者側に移る。
- **短所:** 最小構成を「安全な既定動作」と同義にできない。汎用GUIに組み込むなら実行権限と隔離を別途設計する必要がある。
- **短所:** 会話の木が復元できても、同じ時点の作業ファイルや外部操作が復元されるとは限らない。

## Rust + GPUIへの示唆

自作コアの分解方法として優先して読む。モデルI/O、ツール、セッション、表示、拡張を独立させる。UIには会話の分岐を表示しつつ、ファイルスナップショットIDを別に保持する。

最小ループの実験と製品の安全性は別々に評価する。拡張の便利さを維持しながら、危険操作を止める最終判断はRust側のホストに置く設計が候補となる。

関連: [mini-SWE-agent](swe-agents.md)、[コンテキスト設計](../analysis/context-memory.md)。

[^pi-readme]: [Pi Coding Agent README](https://github.com/earendil-works/pi/tree/main/packages/coding-agent)（参照日: 2026-09-25）。

