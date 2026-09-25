---
type: Research Profile
title: "OpenCode"
description: "HTTPサーバーと複数UI、モデル選択、権限制御を分離した参照例。"
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
  - id: opencode-overview
    resource: "https://opencode.ai/docs/"
    title: "OpenCode Intro"
  - id: opencode-server
    resource: "https://opencode.ai/docs/server/"
    title: "OpenCode Server"
  - id: opencode-permissions
    resource: "https://opencode.ai/docs/permissions/"
    title: "OpenCode Permissions"
  - id: opencode-agents
    resource: "https://opencode.ai/docs/agents/"
    title: "OpenCode Agents"
---

# OpenCode

## 確認できた機能と構造

ターミナル、デスクトップ、IDE向けに提供されるオープンソースのエージェント。複数のモデル提供元を設定できる。[^opencode-overview]
TUIはサーバーのクライアントであり、独立したheadless HTTPサーバーも起動できる。OpenAPI 3.1、SSEイベント、セッションの分岐・中断・差分・要約・復元APIがある。Basic認証を設定できる。[^opencode-server]

Build/Plan等のエージェントとツール権限を設定する。権限はallow/ask/denyに分かれ、確認した既定値では多くがallow。長いコンテキストを要約する内部エージェントも文書化されている。[^opencode-permissions][^opencode-agents]

## 長所・短所の分析

| 観点 | 評価 |
| --- | --- |
| 長所 | UIとheadless APIを分離し、別言語のGUIや複数クライアントを作りやすい |
| 長所 | モデル・エージェント・権限を設定として扱えるため実験しやすい |
| 短所 | HTTP待受、認証、接続寿命、イベント再購読が新しい運用課題になる |
| 短所 | ツールのallow/denyはOS隔離の証明ではない。資料で確認した範囲だけで強いsandbox保証を付けられない |
| 短所 | プロバイダー差を一つのAPIにまとめても、tool callingや文脈長の意味差は残る |

## Rust + GPUIへの示唆

API駆動のUI設計を参考にする。最初はローカルの単一クライアントとし、remote公開を独立した機能として後から設計する。セッションの要約や分岐は、履歴データを上書きせず明示イベントにする。

「Plan」の表示名だけで書き込み禁止を保証しない。実際のツール権限を解決した結果をUIに示す。再利用時はOpenAPIの版、接続先、認証の設定を固定して検証する。

関連: [Codex](codex.md)、[比較表](../analysis/comparison.md)。

[^opencode-overview]: [OpenCode Intro](https://opencode.ai/docs/)（参照日: 2026-09-25）。
[^opencode-server]: [OpenCode Server](https://opencode.ai/docs/server/)（参照日: 2026-09-25）。
[^opencode-permissions]: [OpenCode Permissions](https://opencode.ai/docs/permissions/)（参照日: 2026-09-25）。
[^opencode-agents]: [OpenCode Agents](https://opencode.ai/docs/agents/)（参照日: 2026-09-25）。

