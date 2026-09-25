---
type: Research Profile
title: "Warp"
description: "自然言語とshell操作を連続させるterminal中心のUXの参照例。"
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
  - id: warp-agent
    resource: "https://www.warp.dev/ai"
    title: "Warp Agent Mode"
---

# Warp

## 確認できた機能と構造

Agent Modeはterminalで自然言語と通常のコマンドを扱い、terminal出力等を文脈にして複数段階の作業を進める。利用者がcommandを承認・調整する流れを説明する。現行サイトはTerminal、Agent CLI、Factoriesという入口を区別している。[^warp-agent]

## 長所・短所の分析

- **長所:** 利用者が実行したshellとagentの作業を同じ場所で扱うため、エラーの説明や出力の貼り付けを減らせる。
- **長所:** コード以外のbuild、運用、環境診断へ自然に広がる。
- **短所:** 自然言語とshell入力の自動判定を誤ると、利用者の意図した操作と違う経路へ進む。入力modeの可視化が必要。
- **短所:** terminalには秘密情報や大量ログが現れる。見えている出力をすべてモデルへ送る設計は、費用・文脈・情報制御の面で不利。
- **未確認:** 今回はAgent Mode中心の資料調査であり、Factoriesの詳細、現行実装言語、各製品のlicenseを推測しない。

## Rust + GPUIへの示唆

人の手入力、agent生成command、実行済みcommandを明確に区別する。terminalは単なる文字列表示ではなく、PTY、制御列、終了コード、キャンセル、対話stdinを持つ実行資源として扱う。

UIに表示したログとモデルへ渡すログを別に保持し、渡す範囲を選択可能にする。

関連: [周辺エディター・terminal](../analysis/adjacent-tools.md)、[性能設計](../analysis/performance.md)。

[^warp-agent]: [Warp Agent Mode](https://www.warp.dev/ai)（参照日: 2026-09-25）。

