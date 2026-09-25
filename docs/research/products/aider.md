---
type: Research Profile
title: "Aider"
description: "リポジトリ地図、編集モデル分業、Git連携によるコード変更の参照例。"
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
  - id: aider-map
    resource: "https://aider.chat/docs/repomap.html"
    title: "Repository map"
  - id: aider-git
    resource: "https://aider.chat/docs/git.html"
    title: "Git integration"
  - id: aider-modes
    resource: "https://aider.chat/docs/usage/modes.html"
    title: "Chat modes"
---

# Aider

## 確認できた機能と構造

コードの重要な定義と依存関係からrepo mapを作り、グラフによる選択でトークン予算内に収める。対象ファイル全文を常に送る構成ではない。[^aider-map]
Ask/Code/Architectを使い分け、Architectでは解決案を作るモデルと具体的な編集を作るモデルに役割を分けられる。二段階にする分、呼び出し回数が増える。[^aider-modes]

Git連携には変更の自動commit、既存の未commit変更の扱い、undoがある。自動commitやGit連携は無効化できる。[^aider-git]

## 長所・短所の分析

| 観点 | 長所 | 短所・条件 |
| --- | --- | --- |
| コンテキスト | repo mapにより構造を比較的小さい入力で伝えられる | 動的参照や生成コードを地図だけで理解するのは難しい |
| モデル構成 | 推論と編集を分けて役割ごとに最適化できる | 二段階が総所要時間・費用を悪化させる可能性 |
| 変更管理 | Gitと対話の関係が見えやすい | 自動commitは既存の開発手順・署名・hook方針と衝突し得る |

## Rust + GPUIへの示唆

初期コンテキスト検索は、ファイル選択＋字句検索＋シンボル一覧から始める。embedding基盤を必須にする前に、repo map方式でどこまで解決するか測定する。

patch適用前のファイルhash確認、dirty treeの保護、差分レビューを採用候補とする。Aiderの自動commit方針をそのまま既定値にはしない。ユーザーの作業とエージェントの作業を混同しない変更台帳を先に作る。

適するのは人と往復しながら行うコード修正。多数の外部サービスを使う汎用業務エージェントとしての適性は、この資料だけでは評価しない。

関連: [コンテキスト設計](../analysis/context-memory.md)、[評価計画](../analysis/evaluation.md)。

[^aider-map]: [Repository map](https://aider.chat/docs/repomap.html)（参照日: 2026-09-25）。
[^aider-git]: [Git integration](https://aider.chat/docs/git.html)（参照日: 2026-09-25）。
[^aider-modes]: [Chat modes](https://aider.chat/docs/usage/modes.html)（参照日: 2026-09-25）。

