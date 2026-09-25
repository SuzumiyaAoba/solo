---
type: Research Profile
title: "Roo Code — 提供終了を含む歴史的参照"
description: "役割別モードとオーケストレーション、および製品終了への備えを学ぶ事例。"
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
  - id: roo-docs
    resource: "https://roocodeinc.github.io/Roo-Code/"
    title: "Roo Code official documentation"
---

# Roo Code

## 確認できた機能と現在の状態

公式ドキュメントにはExtensionをMay 15thに終了したという告知があり、ページ更新日は2026-05-15。継続提供中の製品として推奨しない。一方で、公開された設計やソースは歴史的な比較材料になる。[^roo-docs]

資料はモデル非依存のVS Code拡張、filesystem/terminal、MCP、Architect/Code等のcustom Modes、Orchestrator、auto-approveを説明する。Clineから派生したことと、community fork等への案内もある。[^roo-docs]

## 長所・短所の分析

- **長所:** 役割をモードとして分け、プロンプトと利用ツールを一緒に構成する考え方を学べる。
- **長所:** 作業分解と再委任を、単一会話より上の機能として表現した例になる。
- **短所:** agentを増やすほど引継ぎ文脈、費用、競合、失敗時の責任が増える。
- **短所:** 提供終了が明記されており、新規製品の継続依存先には適さない。公開コードの存在と、運営・配布・サポートの継続は別である。
- **未確認:** 案内されるforkの成熟度・互換性は本調査では評価していない。

## Rust + GPUIへの示唆

借りるのはモード定義と状態遷移の考え方。製品依存を避けるため、設定・セッション・添付成果物をexport可能にする。

最初の自作版では単一agentを完成させる。役割ごとの読み取り/書き込み権限は単一agentでも表現できるため、モード切替と並列agentは独立した機能にする。

関連: [Cline](cline.md)、[Kilo](kilo.md)、[継続性と再利用](../analysis/reuse.md)。

[^roo-docs]: [Roo Code official documentation](https://roocodeinc.github.io/Roo-Code/)（参照日: 2026-09-25）。

