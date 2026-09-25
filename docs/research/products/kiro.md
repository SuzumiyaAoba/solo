---
type: Research Profile
title: "Kiro"
description: "仕様・設計・タスクを永続成果物にする開発工程と複数UI共通ハーネスの参照例。"
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
  - id: kiro-docs
    resource: "https://kiro.dev/docs/"
    title: "Kiro Documentation"
  - id: kiro-specs
    resource: "https://kiro.dev/docs/specs/"
    title: "Kiro Specs"
---

# Kiro

## 確認できた機能と構造

公式資料はIDE・CLI・Web・Mobileを共通harnessで動かし、specsやsteering等を共有する構成を説明する。hooks、MCP、permissions、skills、compaction、checkpoint等の資料を公開している。[^kiro-docs]

Specsはrequirementsまたはbugfix、design、tasksというファイルに工程を分ける。featureとbugfixの流れを区別し、task依存関係に基づく並列実行やQuick Specも説明されている。[^kiro-specs]

## 長所・短所の分析

- **長所:** 計画を会話内だけに置かず、利用者が修正・版管理できる成果物として残せる。
- **長所:** 要件、設計、実装、検証の対応を追う仕組みを作りやすい。
- **短所:** 小さい修正で毎回仕様工程を強制すると、入力・レビューの手間が修正そのものを上回る。
- **短所:** 文書と実コードのずれを管理しないと、詳細な仕様が誤った前提のまま残る。
- **短所:** taskの依存が独立でも、同一ファイルや共有環境への書込みが独立とは限らない。

## Rust + GPUIへの示唆

「即時修正」と「仕様から進める作業」を選べる設計が候補。タスクに受入条件、関連diff、実行済み検証を関連付ける。文書を生成するだけで完了とせず、要件ごとに実装・検証の状態を表示する。

並列実行は後期に回す。まず一つのagentでplanを更新・再開できることを確かめる。

関連: [ロードマップ](../analysis/roadmap.md)、[Junie](junie.md)。

[^kiro-docs]: [Kiro Documentation](https://kiro.dev/docs/)（参照日: 2026-09-25）。
[^kiro-specs]: [Kiro Specs](https://kiro.dev/docs/specs/)（参照日: 2026-09-25）。

