---
type: Research Report
title: "Rust + GPUI自作ハーネスのための比較調査レポート"
description: "35の製品・基盤グループの分析から、採用すべき設計と検証順序を導く総合レポート。"
tags: [research, agent-harness]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: comparison
    resource: "analysis/comparison.md"
    title: "横断比較"
  - id: architecture
    resource: "analysis/architecture.md"
    title: "推奨アーキテクチャ"
  - id: evaluation
    resource: "analysis/evaluation.md"
    title: "評価計画"
  - id: reuse
    resource: "analysis/reuse.md"
    title: "再利用・費用・継続性"
---

# Rust + GPUI自作ハーネスのための比較調査

**調査日: 2026-09-25。形式: OKF v0.2。** 詳しい根拠・調査範囲は[調査方法](methodology.md)、個別の機能・長所・短所は[製品索引](products/index.md)を参照する。

## 設計上の結論

推奨は、**小さなRust実行コア、GPUIの表示・操作、独立したtool executor、交換可能なmodel/harness adapter**という分離である。最初は単一agentの編集・検証・中断・再開を完成させ、並列agentやcloud実行を後から追加する。これは公開仕様からの設計提案であり、各実装の速度を測った結論ではない。[^architecture][^evaluation]

「高速」はGPU描画だけでは成立しない。起動、入力応答、context準備、modelの最初の応答、tool実行、圧縮、検証までを分ける。Rust + GPUIが主に改善し得るのは手元のUI・I/O・メモリーの扱いで、remoteモデルの推論時間は別要因となる。[^evaluation]

## 最優先で読む比較対象

| 対象 | 取り入れる設計 | そのまま採用しない前提 |
| --- | --- | --- |
| [Zed・GPUI](products/zed.md) | native UI、Entityの状態、ACP client | フルエディターを最初から再実装する |
| [Codex](products/codex.md) | 構造化server、turn/item、承認イベント | 外部protocolを自作DBの永続schemaにする |
| [Claude Code](products/claude-code.md) | hooks、permission、context、会話/コード復元 | checkpointをshellや外部操作の完全rollbackとみなす |
| [Pi](products/pi.md) | 小さいcore、tree状session、拡張 | 最小機能を安全性の保証とみなす |
| [OpenCode](products/opencode.md) | server/client分離、event API | local HTTPを無認証でremoteへ公開する |
| [goose](products/goose.md)・[Rig](products/rig.md) | Rustでのprovider/tool構成 | Rust製という理由だけで速度と互換性を仮定する |
| [Aider](products/aider.md) | repo map、編集の分業、Gitとの対応 | 全taskに大きな索引や自動commitを強制する |
| [OpenHands](products/openhands.md)・[LangGraph](products/langgraph-deepagents.md) | 実行環境分離、永続状態、interrupt | 初期版から分散基盤を作る |

この優先順位は今回の目的への関連度であり、製品の総合順位ではない。各比較単位の使い分けは[比較表](analysis/comparison.md)に整理した。[^comparison]

## 共通して重要だった機能

1. **構造化された実行状態:** chat文字列だけでは実行中・承認待ち・失敗・結果不明を区別できない。
2. **contextの選択と鮮度:** ファイルの全文投入より、必要箇所の選択、出典、document version、予算管理が重要。
3. **途中の人の介入:** queue、steering、cancel、approval、reviewは異なる操作として表現する。
4. **変更と証拠の対応:** diffだけでなく、実行command、終了code、テスト対象、成果物を関連付ける。
5. **復旧範囲の明示:** session再開、ファイル復元、process再接続、外部副作用の取消しを分ける。
6. **拡張とpolicyの分離:** MCP/skills/pluginの追加でホストの権限が勝手に広がらないようにする。

詳細は[コンテキスト](analysis/context-memory.md)、[安全性と復旧](analysis/security-recovery.md)、[プロトコル](analysis/protocols.md)。

## 取り込みたいUX

Cursor/Devinの成果物レビュー、Clineのdiffと復元、Kiro/Junieの編集可能な計画、Warpのterminal文脈、CopilotのtaskとPRの関係を組み合わせる。機能を全部載せるより、作業先・今していること・変更・検証結果・次に必要な入力を一画面で理解できることを優先する。具体的な分析は各profileに記載した。[^comparison]

## 変更の激しい領域と判断保留

Roo Codeは公式に提供終了を告知。SWE-agentはmini-SWE-agentを推奨。OpenHandsは旧Local GUIと現行Canvasを区別する。Windsurf資料はDevin Desktopへ移行しており、旧称での解説をそのまま適用しない。OpenAI Agents SDKの保守方針には検索表示と取得本文の差があり、確定扱いを避けた。[^reuse]

料金はsubscription、API token、gateway、cloud compute、CI時間、再試行の組合せで変わる。そのため月額の単純順位は作らず、成功taskあたりの費用と利用者のレビュー時間を計測する方針にした。[^reuse][^evaluation]

## 実装へ進む順序

まずGPUIでstream・長いlog・diff・IMEの性能を確認する。次にRustの単一agent loopへread/search/edit/execとpolicyを実装し、クラッシュと中断を注入して復旧を確かめる。外部harnessと固定taskで比較してから、LSP、memory、parallel、remoteへ広げる。

受入条件を含む[ロードマップ](analysis/roadmap.md)と[評価計画](analysis/evaluation.md)に、次の実装作業を分解した。

[^comparison]: [横断比較](analysis/comparison.md)。
[^architecture]: [推奨アーキテクチャ](analysis/architecture.md)。
[^evaluation]: [評価計画](analysis/evaluation.md)。
[^reuse]: [再利用・費用・継続性](analysis/reuse.md)。
