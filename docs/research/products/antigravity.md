---
type: Research Profile
title: "Google Antigravity"
description: "エージェント管理画面、IDE・CLI・SDKの共通ハーネス化を学ぶ参照例。"
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
  - id: antigravity
    resource: "https://antigravity.google/docs/home"
    title: "Google Antigravity Docs"
---

# Google Antigravity

## 確認できた機能と構造

公式資料は独立したdesktop管理画面、CLI、IDE、Python SDKを区別する。Project、複数workspace/worktree、非同期task、scheduled task、ローカルsubagent、MCP/skills、SDKでのtools/hooks/policiesを説明する。共通harnessを複数の入口から使う製品群として位置付けられる。[^antigravity]

## 長所・短所の分析

- **長所:** エディターの横にchatを付ける構成以外に、作業の割当てと結果確認を中心にする画面の参考になる。
- **長所:** GUIとCLI、SDKの能力を共通化しようとする責任分界を検討できる。
- **短所:** 複数workspace、task、agentが同時に動くと、利用者が状況を把握し続ける負担が増す。
- **短所:** モデルとharnessの協調最適化という製品説明は、他のモデルを差し替えて同じ品質を得られる保証ではない。
- **未確認:** Google製品間の履歴移行、全SDK能力のRustからの利用、全サーフェスの挙動一致は未検証。

## Rust + GPUIへの示唆

初期画面でもセッションごとに「作業先」「状態」「未回答の入力」「レビュー待ち」を一覧できるようにする。一方、複数agentを必須にせず、単一agentでも同じ状態モデルを使う。

Gemini CLIとは別profileにした。文書に移行メニューがあっても、終了日や機能の完全同一性を確認できない限り統合扱いにしない。

関連: [Gemini CLI](gemini-cli.md)、[アーキテクチャ](../analysis/architecture.md)。

[^antigravity]: [Google Antigravity Docs](https://antigravity.google/docs/home)（参照日: 2026-09-25）。

