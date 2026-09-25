---
type: Research Profile
title: "OpenAI Agents SDK・Agents API"
description: "自分で運用するagent loopとmanaged harnessの責任分界を比較する事例。"
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
  - id: openai-sdk
    resource: "https://developers.openai.com/api/docs/guides/agents/sdk"
    title: "OpenAI Agents SDK"
  - id: openai-agents-api
    resource: "https://developers.openai.com/api/docs/guides/agents-api/overview"
    title: "OpenAI Agents API"
  - id: openai-selfhost
    resource: "https://developers.openai.com/api/docs/guides/agents-api/environments/self-hosted"
    title: "Agents API self-hosted sandboxes"
---

# OpenAI Agents SDK・Agents API

## 確認できた機能と構造

取得したSDK本文はPython/TypeScript、tool、MCP、guardrails、human review、state、handoff、tracing等を説明する。SDKがループを動かし、アプリケーションが配備・ツール・保存・承認を管理する。[^openai-sdk]

Agents APIはOpenAI側でCodex harnessを動かし、session、orchestration、compaction、recoveryを管理する。[^openai-agents-api]
self-hosted sandboxを選んでも、実行場所とharnessの所在は別。公式説明では手元の環境にexecutorを置き、OpenAI側のharnessからの依頼を処理する。[^openai-selfhost]

## 長所・短所の分析

| 選択肢 | 長所 | 短所・条件 |
| --- | --- | --- |
| Agents SDK | アプリケーション側でtool/state/approvalを組み込める | 配備・永続化・障害復旧の責任が残る。Rust公式SDKを確認したわけではない |
| Agents API | managedなsessionと実行管理を利用できる | ローカル完結やharness内部変更の目的とは異なる |
| ローカルCodex | [App Server](codex.md)経由で実際のコーディングharnessを再利用できる | 外部プロセスとプロトコルへの依存が残る |

## 調査上の不一致

検索エンジンが提示した同じ公式SDK URLには「feature complete・保守中心」という記述があったが、直接取得した本文ではその記述を再確認できなかった。したがって**保守移行を確定した事実として採用しない**。新規採用時に公式support policyとreleaseを再確認する。検索結果だけでSDKを非推奨にしない。

## Rust + GPUIへの示唆

完全な自作harnessなら、自分がloopとpolicyを所有する経路を選ぶ。外部harnessを利用するモードは並立可能だが、Rust側で全挙動を制御できると表示してはいけない。

「local sandbox」を「全データ処理がlocal」と読み替えない。モデル呼出し、control plane、tool実行、保存の所在をそれぞれ表示する。

関連: [責任分界](../analysis/architecture.md)、[費用と継続性](../analysis/reuse.md)。

[^openai-sdk]: [OpenAI Agents SDK](https://developers.openai.com/api/docs/guides/agents/sdk)（参照日: 2026-09-25）。
[^openai-agents-api]: [OpenAI Agents API](https://developers.openai.com/api/docs/guides/agents-api/overview)（参照日: 2026-09-25）。
[^openai-selfhost]: [Agents API self-hosted sandboxes](https://developers.openai.com/api/docs/guides/agents-api/environments/self-hosted)（参照日: 2026-09-25）。

