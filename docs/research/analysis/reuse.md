---
type: Research Analysis
title: "再利用候補・依存・ライセンス・費用・継続性"
description: "直接依存、外部プロセス、設計参照を分け、更新・費用・移行の負担を比較する。"
tags: [research, agent-harness, analysis]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: gpui-profile
    resource: "../products/zed.md"
    title: "Zed・GPUI分析"
  - id: rig-profile
    resource: "../products/rig.md"
    title: "Rig分析"
  - id: goose-profile
    resource: "../products/goose.md"
    title: "goose分析"
  - id: openhands-profile
    resource: "../products/openhands.md"
    title: "OpenHands分析"
  - id: roo-profile
    resource: "../products/roo-code.md"
    title: "Roo Code分析"
  - id: swe-profile
    resource: "../products/swe-agents.md"
    title: "SWE-agent分析"
  - id: openai-profile
    resource: "../products/openai-agents.md"
    title: "OpenAI Agents分析"
  - id: copilot-profile
    resource: "../products/copilot.md"
    title: "Copilot分析"
  - id: acp-rust
    resource: "https://github.com/agentclientprotocol/rust-sdk"
    title: "Official ACP Rust SDK"
  - id: mcp-rust
    resource: "https://github.com/modelcontextprotocol/rust-sdk"
    title: "Official MCP Rust SDK"
  - id: portable-pty
    resource: "https://docs.rs/portable-pty/latest/portable_pty/"
    title: "portable-pty API documentation"
  - id: tree-sitter
    resource: "https://tree-sitter.github.io/tree-sitter/"
    title: "Tree-sitter Introduction"
---

# 再利用と依存判断

## 三つの採用方法

| 方法 | 長所 | 負担 | 今回の使い方 |
| --- | --- | --- | --- |
| libraryとして依存 | 同一processの型・error・性能を揃えやすい | 依存graph、破壊的変更、license | GPUI、Rust SDK等の小さな部品 |
| 外部process/APIとして利用 | エンジンを交換しやすい、既存動作を再利用 | 起動、配布、IPC、認証、version差 | Codex、Pi、OpenCode等の比較backend |
| 設計だけ参照して実装 | coreを自分で所有できる | 実装・保守・評価を自分で担う | session、policy、復旧、UIの独自契約 |

外部processにすればすべての配布条件が無関係になるわけではない。ここでは法的判断を断定せず、依存する実体と契約を特定するための比較に留める。

## Rustで検証する部品

| 部品候補 | 調査で確認した役割 | 採用前に測ること |
| --- | --- | --- |
| GPUI | Rust/GPU UI、Entity/Context | 日本語入力、長い可変高さlist、platform差、API固定 |
| Rig | model/provider/tool/agentのRust型 | 必要なstream、tool引数、usage、cancelを失わないか |
| ACP Rust SDK | editor↔agentのprotocol | v1/v2、unstable機能、対象agentとの組合せ |
| MCP Rust SDK | MCPのRust実装 | spec版、transport、認証、tool変更通知 |
| portable-pty | OSのPTYを抽象化するAPI | process treeの停止、対話stdin、resize、OS差 |
| Tree-sitter | 構文解析の基盤 | 対象言語、grammar版、差分更新、巨大file |

役割の根拠は各一次資料とprofile。全候補を同時に採用する提案ではない。[^gpui-profile][^rig-profile][^acp-rust][^mcp-rust][^portable-pty][^tree-sitter]

gooseはRust実装を読む比較対象、OpenHandsはremote実行の比較対象として優先する。大きいruntime全体を取り込む前に、必要な機能だけを使えるAPI境界を確認する。[^goose-profile][^openhands-profile]

## ライセンスは対象単位で確認する

調査で確認できた表示はGPUI crateのApache-2.0、RigのMIT、gooseのApache-2.0、OpenHands SDKのMIT、ACP Rust SDKのApache-2.0。これは各資料で示された対象に限る。[^gpui-profile][^rig-profile][^goose-profile][^openhands-profile][^acp-rust]

Zed全体、商用cloud、拡張marketplace、SDKが起動する別binaryまで同じ条件とは扱わない。採用releaseを固定した時点で、LICENSE、NOTICE、各crate、grammar、同梱binary、再配布と商用サービスの条件を記録する。今回のレポートはソースを複製して実装していない。

## 更新と継続性

| 対象 | 確認した状態 | 判断への影響 |
| --- | --- | --- |
| Roo Code | 2026-05-15の終了告知 | 新規の継続依存先にしない |
| SWE-agent | mini-SWE-agentを推奨 | 最小baselineは後継を優先 |
| OpenHands | Canvas/SDK/serverと旧Local GUIを区別 | 旧monorepoの構成を前提にしない |
| OpenAI Agents SDK | 検索表示と取得本文で保守方針の差 | 保守終了・移行を断定せず再確認 |
| GPUI / Rig | 破壊的更新への注意を公式に記載 | version固定とadapter内への封じ込め |

根拠は対応する個別分析に残した。[^roo-profile][^swe-profile][^openhands-profile][^openai-profile][^gpui-profile][^rig-profile]

release追従は「自動的に最新版」ではなく、固定→互換試験→更新という手順にする。保存schemaはmigrationを用意し、外部provider型をそのままDBへ保存しない。

## 費用の比較

課金は次の四層に分ける。

1. アプリ利用料・seat・subscription。
2. model token・cache・routing・gatewayの料金。
3. tool・browser・sandbox・VM・CIの利用時間。
4. 自前運用、更新、障害対応、レビューに使う人の時間。

Copilot cloudのようにmodel使用とCI時間の両方が関係する例もある。[^copilot-profile]
BYOKは料金ゼロではなく、model提供元への直接支払経路。local modelも機材、電力、RAM/VRAM、推論待ち時間を必要とする。月額の違いだけで安い・高いとは結論しない。

自作ではrequestごとのusage、予算の残り、unknownな料金を表示する。予約予算と実績を別に持ち、並列実行時には各taskが同じ残額を使えると誤認しない。token単価をアプリ更新と無関係なversion付き設定にして、過去runを当時の単価で再計算できるようにする。

## 移行しやすさ

sessionとartifactはexport可能にし、providerやUIの変更後も読めるようにする。利用者が所有するknowledgeはOKF、実行証跡はJSONL等、内部DBは自作schemaに分ける。特定cloudのIDしか残らない状態を避ける。

関連: [プロトコル](protocols.md)、[ロードマップ](roadmap.md)。

[^gpui-profile]: [Zed・GPUI分析](../products/zed.md)。
[^rig-profile]: [Rig分析](../products/rig.md)。
[^goose-profile]: [goose分析](../products/goose.md)。
[^openhands-profile]: [OpenHands分析](../products/openhands.md)。
[^roo-profile]: [Roo Code分析](../products/roo-code.md)。
[^swe-profile]: [SWE-agent分析](../products/swe-agents.md)。
[^openai-profile]: [OpenAI Agents分析](../products/openai-agents.md)。
[^copilot-profile]: [Copilot分析](../products/copilot.md)。
[^acp-rust]: [Official ACP Rust SDK](https://github.com/agentclientprotocol/rust-sdk)。
[^mcp-rust]: [Official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)。
[^portable-pty]: [portable-pty API documentation](https://docs.rs/portable-pty/latest/portable_pty/)。
[^tree-sitter]: [Tree-sitter Introduction](https://tree-sitter.github.io/tree-sitter/)。

