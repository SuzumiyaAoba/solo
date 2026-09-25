---
type: Research Profile
title: "OpenClaw"
description: "常駐Gateway、複数チャネル、セッション分離、再接続を扱う汎用エージェント基盤。"
tags: [research, agent-harness, gateway]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: official-documentation-review
  hands_on: false
  category: gateway
sources:
  - id: openclaw-docs
    resource: "https://docs.openclaw.ai/"
    title: "OpenClaw documentation"
  - id: openclaw-architecture
    resource: "https://docs.openclaw.ai/concepts/architecture"
    title: "OpenClaw Gateway architecture"
---

# OpenClaw

## 確認できた機能と構造

常駐Gatewayを中心に、複数のmessaging channel、Web Control UI、CLI、device/node、skills、plugin、cron/hooks/webhooksを接続する。会話先・agent・workspaceによるsession分離を扱う汎用agent基盤である。[^openclaw-docs]

Gatewayは認証・pairingと構造化通信を持つ。副作用を持つsend/agent等の要求にはidempotency keyと短時間の重複排除cacheを使う。確認したarchitecture資料ではeventをreplayせず、欠落時はclientが状態を再取得する方針が明記されている。[^openclaw-architecture]

## 長所・短所の分析

- **長所:** 手元のGUIを閉じても続くagent、複数端末からの入力、定期作業をどう一つのruntimeへ集約するか学べる。
- **長所:** device identity、channel、sessionを区別する構造は、将来のremote操作に参考になる。
- **短所:** 常駐processと外部の入力経路が増え、誰のどの要求が、どのworkspaceと権限で実行されるかの管理が重要になる。
- **短所:** eventの再送がない経路では、接続復旧後にsnapshotと実行中taskを照合する必要がある。
- **短所:** 短期dedupe cacheは恒久的なexactly-onceの保証ではない。再起動や保存期間外の再試行を別途扱う。
- **適用範囲:** coding専用editorより、日常の継続的な支援・自動化に重心がある。コード編集のreview UXは別の比較対象と組み合わせる。

## Rust + GPUIへの示唆

初期は単一GUIでも、sessionの寿命をwindowに固定しない設計を検討する。将来daemonへ移す場合は、request受付のack、実行中event、完了結果を別に扱う。

外部channelから来たテキストを直接shell権限へ結び付けず、利用者・channel・workspaceのscopeを明示する。追加チャネルの実装はcore完成後に回す。

関連: [アーキテクチャ](../analysis/architecture.md)、[復旧](../analysis/security-recovery.md)。

[^openclaw-docs]: [OpenClaw documentation](https://docs.openclaw.ai/)（参照日: 2026-09-25）。
[^openclaw-architecture]: [OpenClaw Gateway architecture](https://docs.openclaw.ai/concepts/architecture)（参照日: 2026-09-25）。

