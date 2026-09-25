---
type: Research Profile
title: "Augment Code・Auggie CLI"
description: "大規模コード文脈の索引化とIDE・terminal・自動化の連携を比較する事例。"
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
  - id: augment-docs
    resource: "https://docs.augmentcode.com/introduction"
    title: "Augment Introduction"
  - id: auggie-cli
    resource: "https://docs.augmentcode.com/cli/overview"
    title: "Introducing Auggie CLI"
---

# Augment Code・Auggie CLI

## 確認できた機能と構造

Augmentはコード理解、agent、IDE、automationを組み合わせる開発platform。公式資料はContext Engine、CLI、VS Code/JetBrains、review等の入口を説明する。[^augment-docs]

Auggie CLIはprojectで起動すると索引を作成し、関連repositoryも文脈に利用する。対話TUIに加え、print/quiet形式で自動化に組み込める。確認した資料ではNode環境とAugment accountが必要である。[^auggie-cli]

## 長所・短所の分析

- **長所:** 小さいfile操作だけでなく、広いcodebaseの文脈理解を独立した製品能力として扱う参考になる。
- **長所:** IDEで調べた作業をCLIや自動化へ持ち出す流れを比較できる。
- **短所:** 索引の準備、更新、対象repository、データの所在が速度・情報管理・運用へ影響する。
- **短所:** 索引が豊富でも、最新の未保存bufferや今のbranchを正しく見ているかは別問題。
- **未確認:** 検索の内部algorithm、索引更新の遅延、他製品とのrecall/成功率の差は独立検証していない。

## Rust + GPUIへの示唆

codebase intelligenceをUIから独立したサービスとして扱い、索引の対象・版・最終更新・未反映変更を表示する。

初期版は字句検索とシンボル地図から始め、複数repoやsemantic retrievalが必要になるtaskを測ってから拡張する。headlessの最終文字列だけでなく、途中のtoolとerrorを構造化記録できる接続方式かを確認する。

関連: [Aider](aider.md)、[コンテキスト設計](../analysis/context-memory.md)。

[^augment-docs]: [Augment Introduction](https://docs.augmentcode.com/introduction)（参照日: 2026-09-25）。
[^auggie-cli]: [Introducing Auggie CLI](https://docs.augmentcode.com/cli/overview)（参照日: 2026-09-25）。

