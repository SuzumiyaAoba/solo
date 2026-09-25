---
type: Research Profile
title: "Pydantic AI・Pydantic AI Harness"
description: "型付きtool・出力検証、durable実行、組合せ可能なharness機能の参照例。"
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
  - id: pydantic-overview
    resource: "https://pydantic.dev/docs/ai/overview/"
    title: "Pydantic AI Overview"
  - id: pydantic-harness
    resource: "https://pydantic.dev/docs/ai/harness/"
    title: "Pydantic AI Harness"
---

# Pydantic AI

## 確認できた機能と構造

Pythonの型からtool schemaと構造化出力を定義し、引数をコード実行前に検証する。依存をRunContext経由で渡し、Temporal等を使うdurable実行との統合もある。[^pydantic-overview]

Harness資料はplanning、subagents、background tools、compaction、memory、skills、repo context、guardrails、spend limits等を部品として整理する。Code Modeは複数のtool呼出しをコードでまとめ、途中結果を全て会話へ出さない方式を説明する。[^pydantic-harness]

## 長所・短所の分析

- **長所:** 型とschemaを中心に、provider・tool・出力を一貫して扱う考え方がRustと相性がよい。
- **長所:** 文脈、予算、実行制御を独立した能力として組み合わせやすい。
- **短所:** 構造化出力のschemaが正しくても、値の意味が正しい保証にはならない。
- **短所:** durable基盤やharness拡張をすべて使うと、運用と理解の負担が増える。
- **短所:** Python SDKをRustプロセス内へ直接採用する方式は自然ではなく、別process/APIを検討する必要がある。

## Rust + GPUIへの示唆

serde型、JSON Schema、validationを一つのtool定義から揃える方針を採る。schema検証と権限判定は独立させ、正しいJSONであっても権限外の操作は実行しない。

Code Modeを導入する場合は、toolをまとめて呼ぶ速さと、途中の承認・キャンセル・traceの粒度を両立させる。単なる任意コード実行機能へ置き換えない。

関連: [プロトコル](../analysis/protocols.md)、[性能設計](../analysis/performance.md)。

[^pydantic-overview]: [Pydantic AI Overview](https://pydantic.dev/docs/ai/overview/)（参照日: 2026-09-25）。
[^pydantic-harness]: [Pydantic AI Harness](https://pydantic.dev/docs/ai/harness/)（参照日: 2026-09-25）。

