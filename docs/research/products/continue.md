---
type: Research Profile
title: "Continue"
description: "モデル・ルール・ツールの構成可能性とローカルモデル運用の参照例。"
tags: [research, agent-harness, editor]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: official-documentation-review
  hands_on: false
  category: editor
sources:
  - id: continue-overview
    resource: "https://docs.continue.dev/"
    title: "What is Continue?"
  - id: continue-models
    resource: "https://docs.continue.dev/customize/models"
    title: "Continue Models"
---

# Continue

## 確認できた機能と構造

VS Code/JetBrains向けのオープンソースassistant。Agent、Chat、Edit、AutocompleteとCLIを持ち、models、MCP、rules、promptsを設定する。現行資料では旧context providersや旧@Codebase等にdeprecatedの表示がある。[^continue-overview]

モデルを役割ごとに選べ、ローカルモデルも利用できる。公式のModels資料は、ローカルモデルのtool calling・推論能力によってagent modeが難しくなる場合を説明する。[^continue-models]

## 長所・短所の分析

- **長所:** 補完、会話、編集、agent loopを分けることで、仕事ごとにモデルを選ぶ構成を学べる。
- **長所:** BYOK/ローカル推論を重視する製品設計の参照になる。
- **短所:** 自由度の分だけ、モデル能力、プロンプト、ツールschemaの組合せを利用者側で調整する必要がある。
- **短所:** ローカル推論は、必ずしも低遅延・高品質・低メモリーを意味しない。VRAMとモデル性能の制約を受ける。
- **短所:** 古い検索・context設定例を現行構成へ移す際には移行確認が必要。

## Rust + GPUIへの示唆

モデルを単一の文字列として扱わず、補完・会話・コード編集・要約の役割別profileを持たせる。ただし最初は主モデル一つ＋明示的なfallback程度に絞る。

モデル一覧にはtool support、context上限、structured output等の能力を添える。提供元を切り替える際には履歴中のtool call IDや推論用フィールドの変換を検証する。

関連: [Rig](rig.md)、[コンテキスト設計](../analysis/context-memory.md)。

[^continue-overview]: [What is Continue?](https://docs.continue.dev/)（参照日: 2026-09-25）。
[^continue-models]: [Continue Models](https://docs.continue.dev/customize/models)（参照日: 2026-09-25）。

