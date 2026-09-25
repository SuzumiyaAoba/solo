---
type: Research Profile
title: "Codex — CLI・App Server・SDK"
description: "会話・ターン・実行項目を分離した外部ハーネス接続と承認イベントの参照例。"
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
  - id: codex-cli
    resource: "https://learn.chatgpt.com/docs/codex/cli"
    title: "Codex CLI"
  - id: codex-appserver
    resource: "https://learn.chatgpt.com/docs/app-server"
    title: "Codex App Server"
---

# Codex

評価対象はローカルCLIと、それを独自UIから操作するApp Server。クラウドのAgents APIとは[別の実行配置](openai-agents.md)として扱う。

## 確認できた機能と構造

CLIは作業ディレクトリのコードを読み、編集・コマンド実行を行う対話型エージェントである。[^codex-cli]
App ServerはThread・Turn・Itemを区別し、開始・再開・分岐、途中経過、ツール実行、承認要求を構造化通信で公開する。標準のstdioはJSONL。通信はJSON-RPC 2.0に似るが、wire上では `jsonrpc` ヘッダーを省略する。CLIのバージョンに一致するJSON Schemaを生成できる。[^codex-appserver]

承認はクライアントが応答するサーバー発の要求であり、実行結果とは別イベントになる。WebSocket経路には実験的・非本番向けという注記があるため、stdioと同じ成熟度だと見なさない。[^codex-appserver]

## 長所・短所の分析

| 観点 | 長所 | 短所・条件 |
| --- | --- | --- |
| 独自GUI | 会話ログを解析せず、構造化された実行状態から画面を作れる | プロトコルとイベント順序への追従が必要 |
| 人の介入 | 承認・中断・再開をUIの操作として表現しやすい | 接続断中の承認、期限切れ要求、重複応答をホスト側でも扱う必要 |
| 開発速度 | 既存ハーネスのツール・文脈管理を利用できる | 自作ループの実験自由度は低く、外部エンジンの挙動に依存 |
| 運用 | CLI/GUIで同じ実行機構を共有しやすい | 認証・課金・セッション保存の責任分界を確認する必要 |

## Rust + GPUIへの示唆

最初の外部バックエンド候補。子プロセスとして起動し、標準出力をJSONL専用にする。GPUIの画面状態へ直接プロトコル型を流さず、独自のイベントへ変換する。特に承認要求は `request_id / session_id / turn_id` で関連付け、画面を閉じても黙って許可しない。

実機確認すべき点は、起動時間、再接続後の履歴再取得、ストリーム中断、スキーマ生成物と実際のイベントの一致。ACP経由で接続する場合は、App Server直結との機能差を測る。

関連: [Zed](zed.md)、[プロトコル比較](../analysis/protocols.md)、[推奨アーキテクチャ](../analysis/architecture.md)。

[^codex-cli]: [Codex CLI](https://learn.chatgpt.com/docs/codex/cli)（参照日: 2026-09-25）。
[^codex-appserver]: [Codex App Server](https://learn.chatgpt.com/docs/app-server)（参照日: 2026-09-25）。

