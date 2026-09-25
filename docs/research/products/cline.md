---
type: Research Profile
title: "Cline"
description: "利用者の承認、checkpoint、複数UIとSDKを持つagent coreの参照例。"
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
  - id: cline-overview
    resource: "https://docs.cline.bot/cline-overview"
    title: "Cline Overview"
  - id: cline-checkpoints
    resource: "https://docs.cline.bot/core-workflows/checkpoints"
    title: "Cline Checkpoints"
  - id: cline-approval
    resource: "https://docs.cline.bot/features/auto-approve"
    title: "Cline Auto Approve and YOLO Mode"
---

# Cline

## 確認できた機能と構造

ファイル読書き、shell、browserを扱うコーディングエージェント。VS CodeだけでなくCLI、JetBrains、Kanban、SDKの入口があり、BYOK・ローカルruntimeの利用経路を説明している。[^cline-overview]

Checkpointは各ツール使用後の状態を比較・復元する仕組みで、ファイルだけ、taskだけ、その両方を復元する選択肢を持つ。公式資料には大規模repositoryでcheckpointが性能へ影響し得るとの注意がある。[^cline-checkpoints]
自動承認は操作種類ごとに設定する。確認した資料ではshellの判定にモデルが付ける `requires_approval` が関係し、固定allowlistではない。[^cline-approval]

## 長所・短所の分析

- **長所:** 承認、実行、diff、復元という利用者の制御が画面上で理解しやすい。
- **長所:** agent coreを複数のUIに展開する構造は、GUI/CLI共通コアの比較対象になる。
- **短所:** 頻繁な承認は作業を遅くし、全許可へ振れると確認の意味が失われる。中間となる適切な権限単位が必要。
- **短所:** checkpoint頻度とrepository規模がI/O負荷につながる。安全機能も性能予算に含めなければならない。
- **短所:** モデルの「安全なコマンド」という判断だけでは決定的な隔離境界にならない。

## Rust + GPUIへの示唆

一括自動承認の前に、workspace限定書込み、ネットワーク、外部公開を別の権限にする。保存するのは全treeの毎回複製ではなく、変更前後のcontent hashと必要なblobを中心とする案を測定する。

概要ページの「毎回承認」と自動承認設定の説明は矛盾として放置せず、「初期の操作モデル」と「設定による緩和」を区別してUIへ反映する。

関連: [Roo Code](roo-code.md)、[セキュリティと復旧](../analysis/security-recovery.md)。

[^cline-overview]: [Cline Overview](https://docs.cline.bot/cline-overview)（参照日: 2026-09-25）。
[^cline-checkpoints]: [Cline Checkpoints](https://docs.cline.bot/core-workflows/checkpoints)（参照日: 2026-09-25）。
[^cline-approval]: [Cline Auto Approve and YOLO Mode](https://docs.cline.bot/features/auto-approve)（参照日: 2026-09-25）。

