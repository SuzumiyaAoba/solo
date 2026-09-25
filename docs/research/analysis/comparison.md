---
type: Research Analysis
title: "製品横断比較と選定シナリオ"
description: "35比較単位の機能上の中心、統合負担、適した参照段階と機構の差を比較する。"
tags: [research, agent-harness]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: codex-profile
    resource: "../products/codex.md"
    title: "Codex — CLI・App Server・SDK"
  - id: claude-code-profile
    resource: "../products/claude-code.md"
    title: "Claude Code・Claude Agent SDK"
  - id: gemini-cli-profile
    resource: "../products/gemini-cli.md"
    title: "Gemini CLI"
  - id: opencode-profile
    resource: "../products/opencode.md"
    title: "OpenCode"
  - id: pi-profile
    resource: "../products/pi.md"
    title: "Pi Coding Agent"
  - id: aider-profile
    resource: "../products/aider.md"
    title: "Aider"
  - id: goose-profile
    resource: "../products/goose.md"
    title: "goose"
  - id: crush-profile
    resource: "../products/crush.md"
    title: "Crush"
  - id: zed-profile
    resource: "../products/zed.md"
    title: "Zed・GPUI"
  - id: cursor-profile
    resource: "../products/cursor.md"
    title: "Cursor — Editor・CLI・Cloud Agents"
  - id: devin-desktop-profile
    resource: "../products/devin-desktop.md"
    title: "Windsurf / Devin Desktop・Cascade"
  - id: copilot-profile
    resource: "../products/copilot.md"
    title: "GitHub Copilot — VS Code・CLI・Cloud Agent"
  - id: cline-profile
    resource: "../products/cline.md"
    title: "Cline"
  - id: roo-code-profile
    resource: "../products/roo-code.md"
    title: "Roo Code — 提供終了を含む歴史的参照"
  - id: kilo-profile
    resource: "../products/kilo.md"
    title: "Kilo Code"
  - id: continue-profile
    resource: "../products/continue.md"
    title: "Continue"
  - id: kiro-profile
    resource: "../products/kiro.md"
    title: "Kiro"
  - id: junie-profile
    resource: "../products/junie.md"
    title: "JetBrains Junie"
  - id: antigravity-profile
    resource: "../products/antigravity.md"
    title: "Google Antigravity"
  - id: warp-profile
    resource: "../products/warp.md"
    title: "Warp"
  - id: amp-profile
    resource: "../products/amp.md"
    title: "Amp"
  - id: openhands-profile
    resource: "../products/openhands.md"
    title: "OpenHands — Agent Canvas・SDK・Agent Server"
  - id: devin-profile
    resource: "../products/devin.md"
    title: "Devin"
  - id: openai-agents-profile
    resource: "../products/openai-agents.md"
    title: "OpenAI Agents SDK・Agents API"
  - id: app-builders-profile
    resource: "../products/app-builders.md"
    title: "Replit Agent・Bolt・Lovable"
  - id: langgraph-deepagents-profile
    resource: "../products/langgraph-deepagents.md"
    title: "LangGraph・Deep Agents"
  - id: google-adk-profile
    resource: "../products/google-adk.md"
    title: "Google Agent Development Kit"
  - id: microsoft-agent-framework-profile
    resource: "../products/microsoft-agent-framework.md"
    title: "Microsoft Agent Framework・AutoGen・Semantic Kernel"
  - id: rig-profile
    resource: "../products/rig.md"
    title: "Rig"
  - id: pydantic-ai-profile
    resource: "../products/pydantic-ai.md"
    title: "Pydantic AI・Pydantic AI Harness"
  - id: swe-agents-profile
    resource: "../products/swe-agents.md"
    title: "SWE-agent・mini-SWE-agent"
  - id: crewai-profile
    resource: "../products/crewai.md"
    title: "CrewAI"
  - id: letta-profile
    resource: "../products/letta.md"
    title: "Letta"
  - id: openclaw-profile
    resource: "../products/openclaw.md"
    title: "OpenClaw"
  - id: augment-profile
    resource: "../products/augment.md"
    title: "Augment Code・Auggie CLI"
---

# 製品横断比較

各行のリンク先には機能の一次資料、長所・短所、未確認事項がある。以下の「統合負担」「参照段階」は今回の目的に対する分析であり、実測された製品順位ではない。

## 全比較単位

| 製品・基盤 | 主な入口 | 機能上の中心 | 自作に残る主な仕事・制約 | 適した参照段階 |
| --- | --- | --- | --- | --- | --- |
| [Codex — CLI・App Server・SDK](../products/codex.md)[^codex-profile] | CLI / App Server | 構造化イベント・承認・再開 | 外部runtimeの版とAPIへの依存 | 外部backendの試作 |
| [Claude Code・Claude Agent SDK](../products/claude-code.md)[^claude-code-profile] | CLI / SDK | tool・hooks・contextを統合 | RustとのIPC、復元範囲の把握 | 比較と設計参照 |
| [Gemini CLI](../products/gemini-cli.md)[^gemini-cli-profile] | CLI / ACP | 拡張・policy・sandbox | 実験機能とOS/backend差 | ACP相互運用 |
| [OpenCode](../products/opencode.md)[^opencode-profile] | TUI / server / GUI | HTTP API・SSE・session操作 | 認証、server運用、schema追従 | API型backend |
| [Pi Coding Agent](../products/pi.md)[^pi-profile] | TUI / RPC | 最小core・tree状履歴 | 権限、隔離、拡張の品質保証 | core設計の優先参照 |
| [Aider](../products/aider.md)[^aider-profile] | CLI | repo map・編集・Git | 自動commitと作業手順の調整 | context/patch設計 |
| [goose](../products/goose.md)[^goose-profile] | CLI / GUI / API | Rust runtime・MCP・recipe | 対象crateの分離と適合確認 | Rust source読解 |
| [Crush](../products/crush.md)[^crush-profile] | TUI | LSP・model切替・session | GUI、隔離、復旧の別設計 | 対話UX参照 |
| [Warp](../products/warp.md)[^warp-profile] | terminal / CLI | 人のshellとagentの連続性 | PTY、入力mode、ログ送信範囲 | terminal UX参照 |
| [Zed・GPUI](../products/zed.md)[^zed-profile] | editor / ACP host | GPUI・外部agent表示 | 独自編集機能、API変化への追従 | 最優先UI参照 |
| [Cursor — Editor・CLI・Cloud Agents](../products/cursor.md)[^cursor-profile] | editor / CLI / cloud | browser検証・成果物・steering | local/cloudの契約差 | 作業・レビューUX |
| [Windsurf / Devin Desktop・Cascade](../products/devin-desktop.md)[^devin-desktop-profile] | editor | 現編集状況・plan・revert | 自動contextと復元範囲の表示 | UX参照 |
| [GitHub Copilot — VS Code・CLI・Cloud Agent](../products/copilot.md)[^copilot-profile] | editor / CLI / cloud | IDE連携とIssue→PR | surface別の能力とGitHub依存 | task/review設計 |
| [Cline](../products/cline.md)[^cline-profile] | editor / CLI / SDK | 承認・diff・checkpoint | 承認粒度、snapshotのI/O | 変更レビュー設計 |
| [Roo Code — 提供終了を含む歴史的参照](../products/roo-code.md)[^roo-code-profile] | 終了した拡張 | mode・委任の設計史 | 継続依存には不適 | 歴史的参照 |
| [Kilo Code](../products/kilo.md)[^kilo-profile] | IDE / CLI / gateway | 共有・モデル接続・自動化 | gatewayと各UIの責任分界 | 後期の製品設計 |
| [Continue](../products/continue.md)[^continue-profile] | editor / CLI | 役割別model・local構成 | モデル能力とtoolの組合せ検証 | provider設計 |
| [Kiro](../products/kiro.md)[^kiro-profile] | IDE / CLI / Web | spec→design→task | 小taskの負担、文書とcodeの整合 | 任意の計画workflow |
| [JetBrains Junie](../products/junie.md)[^junie-profile] | IDE / CLI | IDE知識・plan・途中介入 | 言語解析機能との橋渡し | 意味付き編集 |
| [Google Antigravity](../products/antigravity.md)[^antigravity-profile] | desktop / IDE / CLI / SDK | task中心画面・共通harness | 複数workspaceの制御と理解 | session管理UX |
| [Amp](../products/amp.md)[^amp-profile] | CLI / Web / Orb | routing・共有thread | 変化の速い契約、費用の透明性 | remote/routing比較 |
| [OpenHands — Agent Canvas・SDK・Agent Server](../products/openhands.md)[^openhands-profile] | Canvas / SDK / server | UI・runtime・environment分離 | serverとsandboxの運用 | remote段階の比較 |
| [Devin](../products/devin.md)[^devin-profile] | managed開発環境 | 非同期委任・人の引継ぎ | 環境準備、受入条件、実行費用 | 成果物・引継ぎUX |
| [Replit Agent・Bolt・Lovable](../products/app-builders.md)[^app-builders-profile] | managed app builder | 生成→preview→公開 | 任意repository・言語との差 | 成果物UXのみ |
| [OpenAI Agents SDK・Agents API](../products/openai-agents.md)[^openai-agents-profile] | SDK / managed API | 自前loopとmanaged責任の区別 | 実行場所、運用・保存責任 | 要件で方式選択 |
| [LangGraph・Deep Agents](../products/langgraph-deepagents.md)[^langgraph-deepagents-profile] | runtime / harness | checkpoint・interrupt・offload | 抽象の複雑性、副作用の再試行 | 永続実行設計 |
| [Google Agent Development Kit](../products/google-adk.md)[^google-adk-profile] | SDK / workflow | 評価と決定的workflow | coding専用tool/GUIの追加 | 評価・workflow参照 |
| [Microsoft Agent Framework・AutoGen・Semantic Kernel](../products/microsoft-agent-framework.md)[^microsoft-agent-framework-profile] | SDK / harness / workflow | agentとharnessの階層分離 | 世代・言語間の能力差 | 基盤設計の参照 |
| [Rig](../products/rig.md)[^rig-profile] | Rust library | provider/tool/状態の型 | coding executor・policy・GUI | 直接採用のPoC |
| [Pydantic AI・Pydantic AI Harness](../products/pydantic-ai.md)[^pydantic-ai-profile] | SDK / harness | 型・検証・能力の合成 | 意味検証と外部副作用の管理 | 型/guardrail設計 |
| [SWE-agent・mini-SWE-agent](../products/swe-agents.md)[^swe-agents-profile] | 研究用agent | 最小loop・trajectory | 日常UX・細粒度権限 | 品質比較baseline |
| [CrewAI](../products/crewai.md)[^crewai-profile] | Flow / Crew | 決定的工程と委任 | 費用・handoff・書込み競合 | 並列化前の比較 |
| [Letta](../products/letta.md)[^letta-profile] | stateful agent / SDK | 永続memoryとscope | 古い知識・誤記憶の管理 | memory導入時 |
| [OpenClaw](../products/openclaw.md)[^openclaw-profile] | Gateway / CLI / Web | 常駐・channel・device・session | 認証scope、欠落eventの再取得 | 常駐化と再接続設計 |
| [Augment / Auggie](../products/augment.md)[^augment-profile] | IDE / CLI / automation | codebase索引と文脈理解 | 索引の鮮度・範囲・場所 | 大規模repo検索 |

## 同名の機能でも意味が違う

| 論点 | 具体例 | 比較時に確認すること |
| --- | --- | --- |
| 外部UI接続 | Codex App Server、OpenCode HTTP/SSE、Pi RPC、ACP agent | bidirectional承認、中断、再開、履歴取得が全部可能か |
| 計画 | Aider Architect、Kiro Specs、Claude/OpenCodeの制限mode | 推論モデルの役割分離か、文書生成か、実行権限の制限か |
| 復元 | Claudeの会話/ファイル、Cursorのファイル、Piの履歴tree | shell、未追跡file、人の編集、外部APIを含むか |
| 永続実行 | LangGraph、OpenHands、各managed agent | model/toolを再試行した時の副作用をどう扱うか |
| マルチモデル | BYOK、local推論、製品内選択、内部routing | 任意providerを接続できるのか、用意されたmodelだけか |
| MCP対応 | host、client、server、adapter、extension | どちら側を実装し、どのprotocol版・transport・認証を使うか |
| ローカル | local UI、local executor、local model、local storage | 四つが全部localか、一部だけか |
| 並列 | 複数tool、複数session、複数agent、複数worktree | contextだけでなくwrite領域と予算が分離されるか |

## 権限・隔離・復旧の重点比較

| 対象 | 確認された機構 | 誤解しやすい点 |
| --- | --- | --- |
| Claude Code | command sandbox、permission、checkpoint | Bash変更の復元はcheckpointの範囲外 |
| Gemini CLI | sandbox backendとmount設定 | containerの構成により実効的な境界が変わる |
| Copilot CLI | tool許可とpreviewのsandbox | 全tool許可と隔離有効化は別 |
| OpenCode | allow/ask/deny、agent別制御 | tool policyをOS sandboxの代用にしない |
| Cline | 操作別auto-approve、checkpoint | モデルによるcommand判定だけで隔離は成立しない |
| Pi | 最小core、外部隔離・拡張という方針 | project trustと各操作の承認は違う |
| Cursor | local checkpoint、cloudの別環境 | filesystem復元と会話の巻戻しは同じでない |
| LangGraph / Deep Agents | persistence、interrupt、virtual filesystem | 再開、sandbox、exactly-onceは別保証 |

表に載せていない製品の同機能が存在しない、という意味ではない。

## 要件別の絞り込み

| 目的 | 最初に比べる対象 | 理由 | 採用前の判定 |
| --- | --- | --- | --- |
| Rustの独自coreを所有 | Pi、mini-SWE、goose、Rig | 小さいloopとRust部品を比べられる | 同条件taskで成功率と自作負担を測る |
| 高速GUIを先に検証 | Zed/GPUI、Codex App Server、OpenCode | 既存エンジンで表示系を検証できる | 外部agentを置換可能な境界を維持 |
| 汎用agentを組込み | Claude SDK、OpenHands、Deep Agents | toolと文脈管理を再利用できる | model自由度と実行責任を確認 |
| local model重視 | Continue、OpenCode、Rig、goose | モデル接続とローカル経路の参照 | tool calling、VRAM、offline依存を検証 |
| 長い作業の復旧 | LangGraph、OpenHands、Codex | 状態と実行の境界を比較できる | crash直後の二重実行を抑止 |
| 成果物を人がレビュー | Cursor、Cline、Devin、Copilot | diff・test・artifactを結び付ける | レビュー時間と見落としを測る |
| 仕様から実装 | Kiro、Junie | 計画を編集可能な成果物にする | 小taskでの負担と仕様陳腐化を測る |
| 長期memory | Letta、Pydantic AI、Deep Agents | 履歴とmemoryを分ける | source、scope、削除・失効を検証 |

最初に全部を実装しない。今回の初期候補は「Rust単一agent core＋GPUI＋一つの外部harness adapter」。[ロードマップ](roadmap.md)と[評価計画](evaluation.md)で採否を決める。

[^codex-profile]: [Codex — CLI・App Server・SDK](../products/codex.md)。
[^claude-code-profile]: [Claude Code・Claude Agent SDK](../products/claude-code.md)。
[^gemini-cli-profile]: [Gemini CLI](../products/gemini-cli.md)。
[^opencode-profile]: [OpenCode](../products/opencode.md)。
[^pi-profile]: [Pi Coding Agent](../products/pi.md)。
[^aider-profile]: [Aider](../products/aider.md)。
[^goose-profile]: [goose](../products/goose.md)。
[^crush-profile]: [Crush](../products/crush.md)。
[^zed-profile]: [Zed・GPUI](../products/zed.md)。
[^cursor-profile]: [Cursor — Editor・CLI・Cloud Agents](../products/cursor.md)。
[^devin-desktop-profile]: [Windsurf / Devin Desktop・Cascade](../products/devin-desktop.md)。
[^copilot-profile]: [GitHub Copilot — VS Code・CLI・Cloud Agent](../products/copilot.md)。
[^cline-profile]: [Cline](../products/cline.md)。
[^roo-code-profile]: [Roo Code — 提供終了を含む歴史的参照](../products/roo-code.md)。
[^kilo-profile]: [Kilo Code](../products/kilo.md)。
[^continue-profile]: [Continue](../products/continue.md)。
[^kiro-profile]: [Kiro](../products/kiro.md)。
[^junie-profile]: [JetBrains Junie](../products/junie.md)。
[^antigravity-profile]: [Google Antigravity](../products/antigravity.md)。
[^warp-profile]: [Warp](../products/warp.md)。
[^amp-profile]: [Amp](../products/amp.md)。
[^openhands-profile]: [OpenHands — Agent Canvas・SDK・Agent Server](../products/openhands.md)。
[^devin-profile]: [Devin](../products/devin.md)。
[^openai-agents-profile]: [OpenAI Agents SDK・Agents API](../products/openai-agents.md)。
[^app-builders-profile]: [Replit Agent・Bolt・Lovable](../products/app-builders.md)。
[^langgraph-deepagents-profile]: [LangGraph・Deep Agents](../products/langgraph-deepagents.md)。
[^google-adk-profile]: [Google Agent Development Kit](../products/google-adk.md)。
[^microsoft-agent-framework-profile]: [Microsoft Agent Framework・AutoGen・Semantic Kernel](../products/microsoft-agent-framework.md)。
[^rig-profile]: [Rig](../products/rig.md)。
[^pydantic-ai-profile]: [Pydantic AI・Pydantic AI Harness](../products/pydantic-ai.md)。
[^swe-agents-profile]: [SWE-agent・mini-SWE-agent](../products/swe-agents.md)。
[^crewai-profile]: [CrewAI](../products/crewai.md)。
[^letta-profile]: [Letta](../products/letta.md)。
[^openclaw-profile]: [OpenClaw](../products/openclaw.md)。
[^augment-profile]: [Augment Code・Auggie CLI](../products/augment.md)。
