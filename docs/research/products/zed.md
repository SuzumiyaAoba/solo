---
type: Research Profile
title: "Zed・GPUI"
description: "Rust製エディターとACPホスト、GPU描画UIの最重要参照例。"
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
  - id: zed-external
    resource: "https://zed.dev/docs/ai/external-agents"
    title: "Zed External Agents"
  - id: gpui-readme
    resource: "https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md"
    title: "GPUI README"
  - id: gpui-contexts
    resource: "https://github.com/zed-industries/zed/blob/main/crates/gpui/docs/contexts.md"
    title: "GPUI Contexts"
  - id: gpui-manifest
    resource: "https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml"
    title: "GPUI Cargo.toml"
---

# Zed・GPUI

## 確認できた機能と構造

ZedはACP経由で外部エージェントをAgent Panel等へ接続する。外部エージェント側が通常は実行、認証、モデル、ツール、設定を所有する。エディターに登録したAPI keyが外部エージェントにも必ず適用されるわけではない。[^zed-external]

GPUIはRust用のGPU描画フレームワークで、即時モードと保持モードを組み合わせる。READMEにはpre-1.0と破壊的変更の可能性、macOS/Linux系/Windowsそれぞれのbackend設定が記載されている。[^gpui-readme]
EntityのデータはAppが所有し、Contextから更新する。非同期Contextはwindow/appより長生きし得るため、アクセスが失敗し得る。[^gpui-contexts]
確認したgpui crateのlicense表記はApache-2.0。Zedの全crateが同じ条件という意味ではない。[^gpui-manifest]

## 長所・短所の分析

- **長所:** 今回の技術選定に最も近い。エディターとagent runtimeを分離するUX、Rustの状態管理、GPU描画をまとめて参照できる。
- **長所:** ACPによりモデルの変更とハーネスの変更を別の操作にできる。
- **短所:** GPUIを選んでも、成熟したエディター、ターミナル、IME、アクセシビリティが自動で完成するわけではない。
- **短所:** pre-1.0のAPI追従、プラットフォーム依存の描画・文字入力、非同期タスクとwindow寿命の管理が開発負担になる。
- **短所:** 外部agentの設定とホストの設定が分かれるため、権限・認証・費用の所有者がUI上で分かりにくくなり得る。

## Rust + GPUIへの示唆

Zed全体を複製するより、小さな作業画面として「会話・実行履歴・差分・ファイル表示・承認」を作る案を推奨する。GPUIは表示と対話を担い、agent loop、PTY、検索、永続化は独立したサービスにする。

接続時には「このエージェントが使う認証」「実行場所」「設定元」を表示する。GPUIのEntityをバックグラウンド計算の共有可変ストアとして使い回さず、イベントによって投影を更新する。

実測課題は日本語IME、長いログのスクロール、可変高さの会話、GPUメモリー、画面を閉じた後の実行停止・継続である。

関連: [アーキテクチャ](../analysis/architecture.md)、[性能設計](../analysis/performance.md)。

[^zed-external]: [Zed External Agents](https://zed.dev/docs/ai/external-agents)（参照日: 2026-09-25）。
[^gpui-readme]: [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)（参照日: 2026-09-25）。
[^gpui-contexts]: [GPUI Contexts](https://github.com/zed-industries/zed/blob/main/crates/gpui/docs/contexts.md)（参照日: 2026-09-25）。
[^gpui-manifest]: [GPUI Cargo.toml](https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml)（参照日: 2026-09-25）。

