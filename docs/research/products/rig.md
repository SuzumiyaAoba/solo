---
type: Research Profile
title: "Rig"
description: "Rustでprovider・tool・agent状態を扱うための直接的な部品候補。"
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
  - id: rig-repo
    resource: "https://github.com/0xPlaygrounds/rig"
    title: "Rig official repository"
---

# Rig

## 確認できた機能と構造

RigはRustのLLMアプリケーションライブラリー。provider共通のmessage/model/tool、embedding/vector store、streaming、agent runtime等を扱う。現行READMEはprovider/backend契約とorchestrationの分離、直列化可能なAgentRun state machineを説明する。MIT licenseの表示があり、破壊的更新があり得ることも明記される。[^rig-repo]

## 長所・短所の分析

- **長所:** GPUIアプリと同じ言語で統合でき、型、error、非同期処理を揃えやすい。
- **長所:** 各providerのwire形式をすべて自前で書く負担を減らせる可能性がある。
- **短所:** provider抽象が自作に必要な最新機能や細かなイベントを露出しているかは別途確認が必要。
- **短所:** コーディング製品に必要なPTY、Git、承認UI、OS隔離、競合保護がすべて完成済みだという意味ではない。
- **短所:** API更新への追従が必要。依存先の内部型を全アプリへ広げると交換が難しくなる。

## Rust + GPUIへの示唆

直接採用を試す価値があるが、最初はprovider adapter内へ閉じ込める。独自の `ModelEvent`、`ToolCall`、`Usage` へ変換し、Rig型を永続形式にしない。

検証ではpartial tool arguments、複数tool call、stream中断、cancel、usage欠落、provider固有errorを流す。Rust実装であることだけを性能優位の証明にせず、直接HTTP実装と比較する。

関連: [goose](goose.md)、[再利用方針](../analysis/reuse.md)。

[^rig-repo]: [Rig official repository](https://github.com/0xPlaygrounds/rig)（参照日: 2026-09-25）。

