---
type: Research Profile
title: "Kilo Code"
description: "IDE・CLI・モデルgateway・自動化を横断する開発プラットフォームの参照例。"
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
  - id: kilo-docs
    resource: "https://kilo.ai/docs"
    title: "Kilo Documentation"
  - id: kilo-code
    resource: "https://kilo.ai/docs/code-with-ai"
    title: "Kilo Code with AI"
---

# Kilo Code

## 確認できた機能と構造

公式資料はVS Code、JetBrains、CLIに加え、モデルgateway、共有session、チーム向け機能、レビュー等の自動化を案内する。[^kilo-docs]
作業に応じたagentやmodeを使い、コード生成・refactor・debugを行う。[^kilo-code]

## 長所・短所の分析

- **長所:** モデルへの接続、開発UI、共有、運用まで一つの製品群で扱う事例として、製品化後の拡張先が見える。
- **長所:** Gatewayはproviderごとの差をまとめ、利用量や認証を集約する設計候補になる。
- **短所:** gateway、拡張、CLI、クラウド機能の責任範囲が増え、障害原因と費用を分けて追う必要がある。
- **短所:** 一つの画面に機能が掲載されていても、全クライアントで同じ機能が使える証拠にはならない。
- **未確認:** 製品群全体でのセッション互換性、各実行先のOS隔離、checkpointの保証範囲は追加調査が必要。

## Rust + GPUIへの示唆

初期版の必須要件ではなく、後期の拡張性の比較対象。provider adapterと課金gatewayを分離し、自前API key、ローカルモデル、組織gatewayを同じUIから選べる設計にする。

gatewayを介さない構成も残すことで、独立したローカル作業環境という目的を維持できる。外部サービスの名称・所有者より、API契約とexport能力を依存判断の基準にする。

関連: [OpenCode](opencode.md)、[比較表](../analysis/comparison.md)。

[^kilo-docs]: [Kilo Documentation](https://kilo.ai/docs)（参照日: 2026-09-25）。
[^kilo-code]: [Kilo Code with AI](https://kilo.ai/docs/code-with-ai)（参照日: 2026-09-25）。

