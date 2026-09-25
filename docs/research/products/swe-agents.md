---
type: Research Profile
title: "SWE-agent・mini-SWE-agent"
description: "最小実行ループ、環境分離、再現可能な評価を学ぶ研究用基盤。"
tags: [research, agent-harness, framework]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: official-documentation-review
  hands_on: false
  category: framework
sources:
  - id: swe-agent
    resource: "https://swe-agent.com/latest/"
    title: "SWE-agent Getting Started"
  - id: mini-swe
    resource: "https://mini-swe-agent.com/latest/"
    title: "mini-SWE-agent Overview"
---

# SWE-agent・mini-SWE-agent

## 確認できた機能と現在の位置付け

SWE-agentはモデルがtoolを使ってrepositoryの問題を解決する研究用基盤で、YAML設定と実行環境を持つ。公式資料は現在の開発の中心をmini-SWE-agentへ移し、そちらを推奨している。[^swe-agent]

mini-SWE-agentは小さなPython実装、bash中心の操作、線形の会話履歴、各actionを独立したsubprocessで実行する設計を説明する。local、Docker/Podman、Bubblewrap等の環境を選べる。[^mini-swe]

## 長所・短所の分析

- **長所:** 最小のloopが明確で、機能追加の効果を比較する基準実装に向く。
- **長所:** 履歴とモデル入力の対応が理解しやすく、trajectoryを調べやすい。
- **短所:** bashだけの操作は、細粒度の権限制御、意味付きdiff、部分的な操作取消しを作りにくい。
- **短所:** statelessなshell actionでは、前のcommandで変えたcwdや環境変数を暗黙に引き継げない。利用者が期待するterminal sessionとは異なる。
- **短所:** benchmark向けの成功と、日常的なGUIの操作性・安全性・復旧性は別評価になる。

## Rust + GPUIへの示唆

最初の自作loopを比較するbaselineにする。モデル→action→observationの最小経路を再現し、コンテキスト選択や専用edit toolが成功率・時間・費用を改善するか測る。

公式の成功率・起動速度の主張は本調査で再測定していないため、競合順位の根拠にしない。SWE-bench等もモデル・予算・環境が一致する場合に限定して比較する。

関連: [Pi](pi.md)、[評価計画](../analysis/evaluation.md)。

[^swe-agent]: [SWE-agent Getting Started](https://swe-agent.com/latest/)（参照日: 2026-09-25）。
[^mini-swe]: [mini-SWE-agent Overview](https://mini-swe-agent.com/latest/)（参照日: 2026-09-25）。

