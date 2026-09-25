---
type: Research Profile
title: "OpenHands — Agent Canvas・SDK・Agent Server"
description: "コード作業向けagent、UI、サーバー、sandboxを分離する実行基盤の参照例。"
tags: [research, agent-harness, cloud]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: official-documentation-review
  hands_on: false
  category: cloud
sources:
  - id: openhands-overview
    resource: "https://docs.openhands.dev/overview/introduction"
    title: "OpenHands Introduction"
  - id: openhands-sdk
    resource: "https://docs.openhands.dev/sdk"
    title: "OpenHands Software Agent SDK"
---

# OpenHands

## 確認できた機能と構造

現在の構成はAgent Canvas、Python Software Agent SDK、Agent Server、sandbox管理、Cloud/Enterpriseを区別する。Canvasは複数backendに接続するクライアント。Agent Serverは実行・会話・ツール・workspaceをREST/WebSocketで公開する。旧Local GUIはdeprecated、CLIは機能完成・安定性中心と説明されている。[^openhands-overview]

SDKはコード作業に焦点を置き、Bash、編集、browser、MCP、ローカル/remote実行を提供する。SDKページではMIT licenseを明記するが、Enterprise等の別コンポーネントへ同じ条件を広げない。[^openhands-sdk]

## 長所・短所の分析

- **長所:** 自作GUIと実行環境を分離するための比較対象として有力。ローカルからremoteへ移す際の責任分界が見えやすい。
- **長所:** 汎用SDKよりsoftware engineering用のtool/environmentが揃う。
- **短所:** サーバー、sandbox、UIを別々に運用するほど、起動・認証・version整合・資源管理が増える。
- **短所:** 旧monorepo時代の解説を読むと現行コンポーネントと混同しやすい。
- **短所:** Python基盤を直接Rustへ移すより、API境界を設けるほうが保守しやすい場合がある。IPC負担と再実装負担を比較する必要がある。

## Rust + GPUIへの示唆

遠隔実行を追加する段階の比較用backend。UIは「実行場所」を抽象化し、workspaceの実体がローカルpathかremote環境かを明示する。

sessionを再開できても、停止したsandboxが同じfilesystemを提供できるかは別問題。run stateとenvironment lifecycleを別の状態機械にする。

関連: [推奨アーキテクチャ](../analysis/architecture.md)、[評価計画](../analysis/evaluation.md)。

[^openhands-overview]: [OpenHands Introduction](https://docs.openhands.dev/overview/introduction)（参照日: 2026-09-25）。
[^openhands-sdk]: [OpenHands Software Agent SDK](https://docs.openhands.dev/sdk)（参照日: 2026-09-25）。

