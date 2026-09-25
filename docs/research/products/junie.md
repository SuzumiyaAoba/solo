---
type: Research Profile
title: "JetBrains Junie"
description: "IDEのコード理解、計画と実装のモデル分離、途中介入を重視する参照例。"
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
  - id: junie
    resource: "https://junie.jetbrains.com/"
    title: "Junie official product overview"
---

# JetBrains Junie

## 確認できた機能と構造

公式ページはCLI、JetBrains IDE/Android Studio、GitHub、GitLabでの利用を案内する。IDEのcode intelligence、BYOK、構造化されたplan、計画と実装でのモデル選択、Live Prompting、承認、guidelinesとskillsを説明する。[^junie]

## 長所・短所の分析

- **長所:** 言語サーバーだけでは得にくいIDEの知識をagentへ渡す発想が重要。コード操作を単純な文字列置換だけに限定しない。
- **長所:** planをファイルとして残し、実行途中に方向修正する設計は、長い作業の制御に適する。
- **短所:** IDE由来の機能を自作GUIへ移すには、同等の解析サービスまたは外部IDEとの橋渡しが必要。
- **短所:** モデルの分業は低コスト化の可能性がある一方、引継ぎ誤差と追加呼出しを生む。
- **未確認:** 公式ページの費用効率・benchmarkの強調は、今回の固定条件で独立検証していない。

## Rust + GPUIへの示唆

意味を理解したrename・参照探索・診断を提供したいなら、最初からフルIDEを作るよりLSPサービスを独立させる。編集はdocument versionと関連付け、古い診断を最新ファイルへの指摘として表示しない。

強いモデルで計画して速いモデルで編集する案は、同じモデル一つでの実行と成功率・所要時間・費用を比較してから採用する。

関連: [Aider](aider.md)、[評価計画](../analysis/evaluation.md)。

[^junie]: [Junie official product overview](https://junie.jetbrains.com/)（参照日: 2026-09-25）。

