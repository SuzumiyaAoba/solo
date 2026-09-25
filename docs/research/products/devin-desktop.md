---
type: Research Profile
title: "Windsurf / Devin Desktop・Cascade"
description: "コードと会話のモード、編集状況の文脈化、checkpointの参照例。"
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
  - id: cascade
    resource: "https://docs.devin.ai/desktop/cascade/cascade"
    title: "Devin Desktop Cascade Overview"
---

# Windsurf / Devin Desktop・Cascade

## 確認できた機能と構造

調査時、旧WindsurfのCascadeドキュメントはDevin Desktopの公式ページへredirectした。ページはCascadeをDevin Desktop内のローカルエージェントの一つと説明している。旧名と新しい掲載先を同一ファミリーとして追跡する。[^cascade]

Code/Chat、planningとTodo、tool calling、MCP、入力キュー、checkpoint、編集・terminalの選択内容や作業状況の取り込みを持つ。確認した説明ではrevertが不可逆との注記がある。[^cascade]

## 長所・短所の分析

- **長所:** エディターで利用者が今している作業を文脈に含められ、説明を繰り返す手間を減らせる。
- **長所:** 長期計画と直近の操作を分ける表示、名前付きcheckpointは複雑な作業を追いやすくする。
- **短所:** 自動取得した文脈が見えないと、意図していないファイル・選択内容が推論へ混ざる可能性がある。
- **短所:** checkpointやrevertの意味を他製品と同じだと考えられない。取り消しの取り消しが可能かまで製品ごとに確認が必要。
- **短所:** 改称・統合時には既存資料、設定名、利用条件がずれる。過去のWindsurf記事だけでは現行動作を評価できない。

## Rust + GPUIへの示唆

自動コンテキストは「何をいつ取り込んだか」を表示し、送信前に外せるようにする。計画の更新は利用者が編集可能な成果物にする。復元の前には対象差分と復元不能な操作を見せる。

Devinのクラウド実行とCascadeのローカル作業を同じ制御面として扱えるかは未検証。改称だけを理由に内部ハーネスが同一だと断定しない。

関連: [Devin](devin.md)、[コンテキスト設計](../analysis/context-memory.md)。

[^cascade]: [Devin Desktop Cascade Overview](https://docs.devin.ai/desktop/cascade/cascade)（参照日: 2026-09-25）。

