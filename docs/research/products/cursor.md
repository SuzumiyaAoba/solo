---
type: Research Profile
title: "Cursor — Editor・CLI・Cloud Agents"
description: "編集・検索・ブラウザー検証・非同期クラウド作業を統合したUXの参照例。"
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
  - id: cursor-agent
    resource: "https://cursor.com/docs/agent/overview"
    title: "Cursor Agent"
  - id: cursor-cloud
    resource: "https://cursor.com/docs/cloud-agent"
    title: "Cursor Cloud Agents"
---

# Cursor

## 確認できた機能と構造

Agentはコード検索、ファイル編集、terminal、browser等を使う。モデルごとに指示とツールを調整する方針を公式資料が示している。実行中のメッセージ待ち行列、途中の方向修正、コードのcheckpointを持つ。checkpointはGitと別に保存され、復元はファイルを戻しても会話を削除しない。[^cursor-agent]

Cloud Agentsは旧称Background Agents。クラウドで実行し、PRやスクリーンショット、動画、ログ等の成果物を返す。remote desktopで利用者が介入でき、MCPと一部hooksも利用できる。[^cursor-cloud]

## 長所・短所の分析

- **長所:** 変更だけでなく「何を検証したか」を成果物として確認する流れが強い。差分、実行ログ、画面の証拠を一つの作業に結びつけるUXを学べる。
- **長所:** 人が編集する場面と、遠隔で仕事を委任する場面の両方を扱う。
- **短所:** UIが同じでもローカルとクラウドで環境・秘密情報・hooks・利用可能機能が異なる。見た目の統一だけでは設定の移植性は保証されない。
- **短所:** モデルごとの最適化を製品側が担う利便性と、内部の選択を細かく再現できない制約が表裏になる。
- **短所:** 複数agentや大きい文脈の運用では、操作回数だけから費用を予想しにくい。

## Rust + GPUIへの示唆

取り入れるのは、差分から該当ツール実行・テスト証跡へ辿れる設計と、キュー入力と途中指示を区別するUI。UIの高機能化より先に、どのturnがどの変更を生んだかの関連付けを作る。

外部接続候補としてCLI/ACPも調査対象に含めたが、本調査の主な確認範囲はEditor/Cloud Agent。全経路が同じ機能・認証・再開方式を持つとは扱わない。Cloud料金の具体額も比較順位には使用しない。

関連: [Devin](devin.md)、[評価計画](../analysis/evaluation.md)。

[^cursor-agent]: [Cursor Agent](https://cursor.com/docs/agent/overview)（参照日: 2026-09-25）。
[^cursor-cloud]: [Cursor Cloud Agents](https://cursor.com/docs/cloud-agent)（参照日: 2026-09-25）。

