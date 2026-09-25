---
type: Research Profile
title: "GitHub Copilot — VS Code・CLI・Cloud Agent"
description: "エディター内作業、terminal、IssueからPRまでの委任を比較する製品群。"
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
  - id: vscode-agents
    resource: "https://code.visualstudio.com/docs/agents/overview"
    title: "Build with AI in VS Code"
  - id: copilot-cli
    resource: "https://docs.github.com/en/copilot/concepts/agents/copilot-cli/about-copilot-cli"
    title: "About GitHub Copilot CLI"
  - id: copilot-cloud
    resource: "https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent"
    title: "About GitHub Copilot cloud agent"
---

# GitHub Copilot・VS Code

## 確認できた機能と構造

VS Codeの現行資料は、モデル、agent harness、toolを別々の選択軸として説明する。Chat viewとAgents window、複数session、計画、変更・テスト、他のagent提供元の統合を扱う。エディター自体とCopilotのハーネスは区別すべきである。[^vscode-agents]

Copilot CLIは対話とpromptによる自動実行、ツール許可、GitHub/MCP連携を持つ。確認した資料ではローカル・クラウドsandboxはpublic preview。ツールを全許可することはsandboxの有効化とは異なる。[^copilot-cli]

Cloud AgentはGitHub上の作業を委任し、PR等で結果を受け取る。資料上、一実行の変更先は開始時に指定した一リポジトリで、一branch・一PRという制約がある。使用量にはAI creditsとActions minutesが関係する。[^copilot-cloud]

## 長所・短所の分析

| 利用形態 | 長所 | 短所・条件 |
| --- | --- | --- |
| VS Code | 診断、debugger、テスト、diffと同じ画面で作業できる | エディター拡張やprovider設定の相互作用が増える |
| CLI | shellとGitHub操作を一つの対話で扱える | preview機能を安定した隔離保証として扱えない |
| Cloud | Issue→変更→CI→PRのレビュー過程に組み込みやすい | GitHubの環境・権限・実行制約に依存する |

## Rust + GPUIへの示唆

同じ画面に複数の実行先を表示しても、実行場所、作業tree、使用モデル、費用の所有者は区別する。ローカル作業をクラウドへ引き継ぐ操作では、履歴だけでなく未commit変更と環境定義の扱いも確認する。

自作で再現する価値が高いのは、taskとreviewの結び付け。GitHub固有の操作はGitHub adapterとして独立させ、コアのセッション形式へ埋め込まない。

関連: [OpenHands](openhands.md)、[比較表](../analysis/comparison.md)。

[^vscode-agents]: [Build with AI in VS Code](https://code.visualstudio.com/docs/agents/overview)（参照日: 2026-09-25）。
[^copilot-cli]: [About GitHub Copilot CLI](https://docs.github.com/en/copilot/concepts/agents/copilot-cli/about-copilot-cli)（参照日: 2026-09-25）。
[^copilot-cloud]: [About GitHub Copilot cloud agent](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/about-cloud-agent)（参照日: 2026-09-25）。

