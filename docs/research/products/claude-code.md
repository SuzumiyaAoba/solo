---
type: Research Profile
title: "Claude Code・Claude Agent SDK"
description: "ツール・権限・文脈管理を統合したハーネスとSDK化の参照例。"
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
  - id: claude-overview
    resource: "https://code.claude.com/docs/en/overview"
    title: "Claude Code overview"
  - id: claude-sdk
    resource: "https://code.claude.com/docs/en/agent-sdk/overview"
    title: "Claude Agent SDK overview"
  - id: claude-sandbox
    resource: "https://code.claude.com/docs/en/sandboxing"
    title: "Configure the sandboxed Bash tool"
  - id: claude-checkpoint
    resource: "https://code.claude.com/docs/en/checkpointing"
    title: "Checkpointing"
---

# Claude Code・Claude Agent SDK

## 確認できた機能と構造

Claude CodeはCLIを中心に、コード操作、ターミナル、MCP、指示・メモリー、拡張機構を組み合わせる。Agent SDKは同じループ・組み込みツール・コンテキスト管理をPython/TypeScriptから利用するライブラリーで、Claude Codeバイナリーを動かす。SDKにはセッション、hooks、承認、使用量計測がある。[^claude-overview][^claude-sdk]

Bash sandboxはファイルシステムとネットワークを制限する。確認した資料ではmacOS/Linux/WSL2を対象とし、ネイティブWindowsの同機能は未対応。隔離外での再試行や隔離を開始できない場合の挙動は設定に依存する。[^claude-sandbox]

Checkpointは会話とファイルを別々に復元できる。ただしBash経由の変更は追跡対象外で、サブエージェントの変更にも復元範囲の制約がある。完全なOSスナップショットではない。[^claude-checkpoint]

## 長所・短所の分析

- **長所:** 既存ハーネスをSDK経由で使うため、ツール設計・コンテキスト管理・承認を一から組む負担が小さい。hooksは決定的な処理を挿入する境界として参考になる。
- **長所:** 会話の巻き戻しとコードの巻き戻しを区別するUIは、試行錯誤の支援に適している。
- **短所:** 汎用のマルチプロバイダー実行機構として設計する場合、Claudeのループ・配布バイナリーへの依存が残る。SDKの公開言語とRustの間にIPC等が必要。
- **短所:** 「許可」「隔離」「復元」の三つは異なる保証。チェックポイントがあるから任意のshell操作まで安全に戻せる、とは言えない。

## Rust + GPUIへの示唆

SDKを比較用バックエンドとして別プロセスで動かす方法と、公開CLIの構造化出力を使う方法を評価する。独自コアでは `BeforeTool / ApprovalRequested / ToolFinished` を分離し、フックのタイムアウト・失敗時方針も定義する。

復元UIでは、ファイル編集、shell変更、外部API、会話状態のどこまで戻るかを表示する。指示ファイルは権限設定の代用にしない。

関連: [セキュリティと復旧](../analysis/security-recovery.md)、[Pi](pi.md)。

[^claude-overview]: [Claude Code overview](https://code.claude.com/docs/en/overview)（参照日: 2026-09-25）。
[^claude-sdk]: [Claude Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview)（参照日: 2026-09-25）。
[^claude-sandbox]: [Configure the sandboxed Bash tool](https://code.claude.com/docs/en/sandboxing)（参照日: 2026-09-25）。
[^claude-checkpoint]: [Checkpointing](https://code.claude.com/docs/en/checkpointing)（参照日: 2026-09-25）。

