---
type: Research Analysis
title: "ハーネス・エージェント・エディター・SDKの責任分界"
description: "比較対象を同じ階層で評価するための用語と設計軸を整理する。"
tags: [research, agent-harness, analysis]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
research:
  as_of: 2026-09-25
  method: evidence-based-analysis
  hands_on: false
sources:
  - id: codex-profile
    resource: "../products/codex.md"
    title: "Codex分析"
  - id: zed-profile
    resource: "../products/zed.md"
    title: "Zed分析"
  - id: langgraph-profile
    resource: "../products/langgraph-deepagents.md"
    title: "LangGraph・Deep Agents分析"
  - id: openhands-profile
    resource: "../products/openhands.md"
    title: "OpenHands分析"
---

# 比較のための用語と責任分界

## このレポートでの定義

| 層 | 主な責任 | 比較例 |
| --- | --- | --- |
| Model | 入力から推論・出力・tool callを生成 | 同じharnessでもモデルが変わる |
| Agent definition | 目標、指示、model、利用tool、出力形式 | roleやmodeの設定 |
| Harness | loop、文脈、tool routing、承認、予算、状態、停止・再開 | Codex、Claude Code、Pi、Deep Agents |
| Workflow runtime | 決定的な分岐、待機、再開、checkpoint | LangGraph、CrewAI Flow |
| Execution environment | filesystem、process、network、browserを実際に動かす | local executor、container、remote machine |
| Client / editor | 入力、表示、diff、レビュー、人の介入 | Zed、VS Code、独自GPUI |
| Protocol / extension | 境界間の情報交換や手順追加 | ACP、MCP、skills |

この分類は本レポートの整理。製品は複数層を同時に実装する。Codexの構造化server、Zedの外部agent、LangGraphとDeep Agentsの層分け、OpenHandsのUI/server/environment分割を参考にした。[^codex-profile][^zed-profile][^langgraph-profile][^openhands-profile]

## 混同すると設計を誤る点

1. **モデルの交換とharnessの交換:** 同じモデルでも、指示、tool、検索、context選択、停止条件が違えば結果が変わる。
2. **CLIとエンジン:** terminalで動くことは実行機構がUIと密結合している証拠ではない。RPCやheadless APIがあれば独自GUIから使える。
3. **権限と隔離:** ユーザーに許可を求める画面と、OSで操作を制限する仕組みは別。
4. **会話とworkspace:** 会話を復元しても、ファイル、プロセス、外部サービスの状態は戻らない。
5. **永続実行と副作用の一回性:** 状態保存があっても、再試行で外部操作が二重になる可能性は残る。
6. **開放されたsourceと安定したAPI:** コードが読めても、内部crateを安定依存にできるとは限らない。

## 今回の自作範囲

推奨する中心は「自分で所有するRust harness」と「GPUI client」。外部harness adapterは比較・相互運用のために併設する。外部agentを表示するだけの段階は試作として有用だが、それだけを自作harnessの完成とは定義しない。

最初に共通化すべきなのは会話message型だけではなく、tool実行、承認、成果物、費用、失敗を表現するeventである。詳細は[アーキテクチャ](architecture.md)。

[^codex-profile]: [Codex分析](../products/codex.md)。
[^zed-profile]: [Zed分析](../products/zed.md)。
[^langgraph-profile]: [LangGraph・Deep Agents分析](../products/langgraph-deepagents.md)。
[^openhands-profile]: [OpenHands分析](../products/openhands.md)。

