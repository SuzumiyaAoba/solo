---
type: Research Profile
title: "Crush"
description: "Go製TUIとLSP・MCP・複数モデルを組み合わせた対話操作の参照例。"
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
  - id: crush-readme
    resource: "https://github.com/charmbracelet/crush"
    title: "Crush README"
---

# Crush

## 確認できた機能と構造

Goで配布・実行できるターミナル型コーディングエージェント。複数のモデル提供元、文脈を保ったモデル切替、プロジェクト別セッション、LSPからの追加コンテキスト、MCP拡張を公式READMEで確認した。[^crush-readme]

## 長所・短所の分析

- **長所:** キーボード中心の操作、プロジェクト内のセッション管理、モデル選択を一つのTUIにまとめる設計を参照できる。
- **長所:** LSPをエージェントの情報源に使う方針は、字句検索だけでは分からない診断・定義情報の利用につながる。
- **短所:** LSP起動や言語ごとの設定に費用がかかる。単に接続できるだけでは、診断の鮮度や編集済みbufferとの整合は保証されない。
- **短所:** ターミナル表示で完成しているUXをGPUIへ移す際には、レイアウト、IME、アクセシビリティ、選択・コピーの設計を改めて行う必要がある。
- **未確認:** native SDKとしてRustへ組み込む安定した契約、全ツールを覆うOS隔離、クラッシュ復旧保証は本調査で検証していない。

## Rust + GPUIへの示唆

実行エンジンの採用候補というより、入力・セッション切替・進捗の表示密度の参考にする。LSPはUIとagentが別々に起動せず、共通サービス経由で最新のdocument versionを付けて提供する。

関連: [Zed](zed.md)、[性能設計](../analysis/performance.md)。

[^crush-readme]: [Crush README](https://github.com/charmbracelet/crush)（参照日: 2026-09-25）。

