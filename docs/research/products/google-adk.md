---
type: Research Profile
title: "Google Agent Development Kit"
description: "agent、決定的workflow、評価、配備を分離する汎用基盤の参照例。"
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
  - id: google-adk
    resource: "https://adk.dev/"
    title: "Google ADK official documentation"
---

# Google ADK

## 確認できた機能と構造

ADKはagentの構築、debug、評価、配備を扱うオープンソースframework。現行ページは複数言語のSDKと、決定的なコードとモデル推論を組み合わせるgraph workflowを説明する。[^google-adk]

## 長所・短所の分析

- **長所:** agent実装だけでなく、評価と配備を開発工程に含めて考えられる。
- **長所:** すべての処理をモデルへ委任せず、既知の処理はコードで制御する構成の参考になる。
- **短所:** 汎用基盤なので、コードdiff、Git、PTY、編集競合といったdesktop coding製品の要件は別途必要。
- **短所:** SDKが複数言語に存在しても、機能の完全な一致を前提にできない。
- **未確認:** 今回は各言語のAPI差、全providerの対応、配備サービスごとの制約を実機比較していない。

## Rust + GPUIへの示唆

コア採用より評価・workflow設計を参照する。formatter、path検証、patch適用、hash計算、予算判定は決定的なRustコードで処理し、モデルには判断が必要な部分を担当させる。

SDKの「利用可能な言語」を、そのまま自作Rustアプリへの低コストな組込み可能性と読み替えない。protocolやサービス境界の追加費用を評価する。

関連: [Microsoft Agent Framework](microsoft-agent-framework.md)、[評価計画](../analysis/evaluation.md)。

[^google-adk]: [Google ADK official documentation](https://adk.dev/)（参照日: 2026-09-25）。

