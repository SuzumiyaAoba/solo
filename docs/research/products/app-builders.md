---
type: Research Profile
title: "Replit Agent・Bolt・Lovable"
description: "アプリ生成・プレビュー・公開を一体化する製品群から成果物中心のUXを学ぶ。"
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
  - id: replit-agent
    resource: "https://docs.replit.com/features/agent/overview"
    title: "Replit Agent"
  - id: bolt-intro
    resource: "https://support.bolt.new/get-started/intro-bolt"
    title: "Intro to Bolt"
  - id: lovable-intro
    resource: "https://docs.lovable.dev/introduction/welcome"
    title: "Welcome to Lovable"
---

# Replit Agent・Bolt・Lovable

これらは汎用ローカルharnessの直接代替ではなく、成果物を素早く確認するUXの比較対象である。

## 製品ごとの機能・長所・短所

| 製品 | 確認できた機能 | 長所の分析 | 短所・適用範囲の分析 |
| --- | --- | --- | --- |
| Replit Agent | コード、環境構築、テスト、plan、checkpoint、公開までの流れ | 環境を準備する手間と確認までの距離が小さい | 同じ環境・配備基盤への依存があり、任意のローカルrepositoryとは条件が異なる |
| Bolt | JavaScript系full-stack、Expo、code view、外部連携、hosting等 | prompt→preview→編集→公開の往復を作りやすい | Rust/任意言語の汎用作業への適性をWebアプリ生成の強さから推測できない |
| Lovable | 自然言語によるfrontend/backend/database/auth生成、共有workspace、Git同期 | 実装を直接読まない人でも成果を確認しやすい | 生成、運用、認証、DB等が一体なので、部分的な採用と移行の境界を確認する必要 |

機能の根拠は各社の公式概要。性能や生成物の品質を横並びに測定した表ではない。[^replit-agent][^bolt-intro][^lovable-intro]

## Rust + GPUIへの示唆

最も有用なのは「作業結果をすぐに触れる」体験。自作でも、生成ファイル、起動したpreview URL、スクリーンショット、テスト結果をtaskの成果物としてまとめる。

デプロイは編集やローカルpreviewより強い外部効果を持つため、独立した操作にする。最初からhostingやDBを製品へ内蔵する必要はない。

関連: [Devin](devin.md)、[評価計画](../analysis/evaluation.md)。

[^replit-agent]: [Replit Agent](https://docs.replit.com/features/agent/overview)（参照日: 2026-09-25）。
[^bolt-intro]: [Intro to Bolt](https://support.bolt.new/get-started/intro-bolt)（参照日: 2026-09-25）。
[^lovable-intro]: [Welcome to Lovable](https://docs.lovable.dev/introduction/welcome)（参照日: 2026-09-25）。

