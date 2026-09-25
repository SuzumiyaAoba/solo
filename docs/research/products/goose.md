---
type: Research Profile
title: "goose"
description: "Rust製の実行基盤、MCP拡張、再利用可能なrecipeの参照例。"
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
  - id: goose-repo
    resource: "https://github.com/aaif-goose/goose"
    title: "goose official repository"
  - id: goose-recipes
    resource: "https://goose-docs.ai/docs/guides/recipes/"
    title: "goose Recipes"
---

# goose

## 確認できた機能と構造

ローカルで動作する汎用エージェントで、デスクトップ・CLI・APIを提供する。公式READMEはRust実装、複数のモデル提供元、MCP拡張、ACP経由の接続を説明している。コード作業だけでなく調査・自動化等を対象とする。現在の公式リポジトリはaaif-goose/gooseで、Apache-2.0の表示がある。[^goose-repo]

Recipeはプロンプト、拡張、設定をまとめた再利用可能なワークフロー。パラメーター化した実行手順を共有できる。[^goose-recipes]

## 長所・短所の分析

- **長所:** Rust + GUI + CLI + 外部ツールという今回の構想に近い。SDKだけでなく、実際の利用形態を含めて設計を調べられる。
- **長所:** Recipeは「会話のやり直し」を「再実行可能な作業定義」に変える材料になる。
- **短所:** 汎用性が高い分、コード編集専用UIやGitの細かな操作を追加する必要が生じる可能性がある。
- **短所:** MCP拡張が増えると起動、スキーマ読込、認証、ツール名衝突、モデル入力が増える。接続数はそのまま品質の指標にならない。
- **未確認:** 本調査ではリポジトリ全crateの依存関係とGUI実装を精査していない。「Rust製」から全UIがGPUI製だとは推測しない。

## Rust + GPUIへの示唆

Rust実行基盤を新規実装する前のコード読解候補。ツール登録、provider境界、CLIとAPIの共用方法を確認する。ソース再利用は対象crate単位の依存・ライセンス確認後に決める。

Recipe相当の宣言にはモデル選択だけでなく、必要権限、入力、出力、終了条件、予算を含める。再実行は同じ結果の保証ではないため、毎回のモデル・ツール版と実行証跡を保存する。

関連: [Rig](rig.md)、[再利用と依存関係](../analysis/reuse.md)。

[^goose-repo]: [goose official repository](https://github.com/aaif-goose/goose)（参照日: 2026-09-25）。
[^goose-recipes]: [goose Recipes](https://goose-docs.ai/docs/guides/recipes/)（参照日: 2026-09-25）。

