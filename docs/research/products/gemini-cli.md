---
type: Research Profile
title: "Gemini CLI"
description: "Google系CLIの拡張・隔離・ACP対応を比較する参照例。"
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
  - id: gemini-docs
    resource: "https://geminicli.com/docs/"
    title: "Gemini CLI documentation"
  - id: gemini-sandbox
    resource: "https://geminicli.com/docs/cli/sandbox/"
    title: "Sandboxing in Gemini CLI"
  - id: gemini-acp
    resource: "https://geminicli.com/docs/cli/acp-mode/"
    title: "ACP Mode"
---

# Gemini CLI

## 確認できた機能と構造

公式ドキュメントはshell・Web取得・MCP、GEMINI.md、skills、hooks、セッション、checkpoint、headless、policy engine、ACP modeを扱う。機能一覧に実験的な項目も含まれるため、掲載されていることと安定提供は区別する。[^gemini-docs]
ACP modeはエディターとCLIを接続する入口になる。[^gemini-acp]

Sandboxにはコンテナー等の実行方式、ワークスペース外のmount設定、権限拡張がある。mountとネットワーク設定により実際の境界が変わる。Docker socketを共有する構成を、独立したVMと同じ隔離強度と解釈しない。[^gemini-sandbox]

## 長所・短所の分析

- **長所:** CLI、headless、ACPの三つを比べられ、単一エンジンを異なるクライアントで使う設計を学びやすい。
- **長所:** プロジェクト指示、拡張配布、ポリシー、sandboxを別の機能として整理できる。
- **短所:** モデル・認証・利用枠をGoogle系の製品関係と合わせて理解する必要がある。無料枠の存在だけで総費用の優劣は決まらない。
- **短所:** 実験機能、OS差、sandbox backend差を一つの「対応」表示にまとめると、実際に使える範囲を誤る。

## Rust + GPUIへの示唆

ACPクライアントの相互運用テスト候補とする。機能はモデル名で推測せず、接続時に得た能力とアダプターの対応表から有効化する。GEMINI.mdの互換読み込みを追加する場合、AGENTS.mdとの優先順位を明示する。

[Antigravity](antigravity.md)にもCLI/SDKがあるが、この調査では両者の全機能同一性やGemini CLIの終了を確認していない。名前が似たGoogle製品を同一ハーネスと数えない。

関連: [プロトコル比較](../analysis/protocols.md)。

[^gemini-docs]: [Gemini CLI documentation](https://geminicli.com/docs/)（参照日: 2026-09-25）。
[^gemini-sandbox]: [Sandboxing in Gemini CLI](https://geminicli.com/docs/cli/sandbox/)（参照日: 2026-09-25）。
[^gemini-acp]: [ACP Mode](https://geminicli.com/docs/cli/acp-mode/)（参照日: 2026-09-25）。

