---
type: Research Analysis
title: "コンテキスト・圧縮・メモリーの設計"
description: "検索、指示、tool出力、長期知識を分け、予算と鮮度を管理する方法を比較する。"
tags: [research, agent-harness, analysis]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: aider-profile
    resource: "../products/aider.md"
    title: "Aider分析"
  - id: pi-profile
    resource: "../products/pi.md"
    title: "Pi分析"
  - id: letta-profile
    resource: "../products/letta.md"
    title: "Letta分析"
  - id: pydantic-profile
    resource: "../products/pydantic-ai.md"
    title: "Pydantic AI分析"
  - id: skills-spec
    resource: "https://agentskills.io/specification"
    title: "Agent Skills Specification"
---

# コンテキストとメモリー

Aiderのrepo map、Piの分岐履歴、Lettaの永続memory、Pydantic AIの能力別context管理は、それぞれ違う問題を解く。これらを一つの「記憶」機能にまとめない。[^aider-profile][^pi-profile][^letta-profile][^pydantic-profile]

## 保存対象を分ける

| 種類 | 寿命とscope | 必要な保証 |
| --- | --- | --- |
| user要求と制約 | 現task / session | 圧縮後も目的・禁止事項・完了条件を保持 |
| project指示 | repository / directory | どのpathの指示を適用したか記録 |
| 選択されたコード | 現turn | path、範囲、buffer/disk version、hash |
| tool観測 | 実行単位 | 完全な原本とmodel向け抜粋を区別 |
| 作業メモ | session / project | source event、更新者、失効条件 |
| 長期memory | user / project | 明示scope、訂正・削除、信頼度 |
| 検索索引 | repositoryの補助cache | 現在のfile内容との整合、再構築可能 |
| prompt cache | provider依存の最適化 | 秘密・tenant境界、cache hitの計測 |

## 検索の導入順序

初期は明示file指定、字句検索、file tree、差分、診断を中心にする。次にTree-sitter/LSP等から定義・参照の要約を作り、repo mapの効果を試す。embedding検索は必要性が測れた段階で加える。

| 方式 | 長所の分析 | 弱点の分析 | 適する質問 |
| --- | --- | --- | --- |
| file/rg型検索 | 準備が小さく、正確な文字列に強い | 別名・言い換えに弱い | error名、関数名、設定key |
| シンボル/依存地図 | 構造を短く示せる | 動的解決・生成codeに限界 | 入口、依存関係、呼出し先 |
| embedding | 語の一致しない説明を拾える | 索引費用、古さ、誤関連 | 概念から実装を探す |
| hybrid＋rerank | 複数の手掛かりを組合せ | 調整と実行費用が増える | 大規模repoの曖昧な探索 |

検索の比較は正解file/範囲が既知のtaskで行い、最終成功率も見る。検索hitが多いことを品質とみなさない。

## トークン予算を一つの数字にしない

予約する領域は、上位指示、現在のtask、直近の重要会話、必要コード、tool schema、tool結果、生成余裕に分かれる。余裕がなくなった時、最新の要求を削るのではなく、古い大きなtool結果や重複引用を先に退避する案を採る。

Skillsのmetadataだけを先に読み、必要時に本体とresourceを開く方式は入力の肥大化を抑える参考になる。[^skills-spec]
MCPも全部のtool説明を無条件に毎回渡さず、能力の検索・遅延ロードを評価する。ただし、toolを探すために必要な説明まで削らない。

## tool出力の二重表現

監査用原本はartifactへ保存し、model向けには上限付きexcerptを渡す。excerptには終了code、切り詰めの有無、総byte数、原本の参照、抜粋したoffsetを含める。

例えば長いbuild logでは最後の200行だけでなく、最初のerrorと関連箇所を抽出する。抽出が正しいとは限らないため、modelが原本を追加取得できるようにする。利用者の画面でも切り詰めを隠さない。

## 圧縮時の契約

圧縮は「古い会話を短くする操作」であって、達成すべきtaskを再定義する操作ではない。要約には次を明示する。

- 元eventの範囲と、対象外に残したmessage。
- 現在の目的、利用者の修正、許可と禁止、採用した判断。
- 変更済みfile、実行済みtest、未解決error、結果不明の副作用。
- 次の手順と、必要なら再取得するartifact。

完全履歴は残す。圧縮前後で「branchを変えない」「subagentを使わない」等の制約を維持できるかをテストする。これは今回の調査作業でも守っている条件である。

## memoryの書込みは知識の確認と分ける

agentが推測した内容を、そのまま永続的なproject事実へ昇格しない。source、scope、作成過程、確認状態、stale_afterを持つ記録として保存する。

今回のOKF bundleは設計資料の交換に向くが、実行ログDBの代用ではない。taskの内部状態は構造化eventで保存し、利用者へ残す知識や分析をOKFとしてexportする構成が候補になる。

関連: [安全性と復旧](security-recovery.md)、[性能](performance.md)。

[^aider-profile]: [Aider分析](../products/aider.md)。
[^pi-profile]: [Pi分析](../products/pi.md)。
[^letta-profile]: [Letta分析](../products/letta.md)。
[^pydantic-profile]: [Pydantic AI分析](../products/pydantic-ai.md)。
[^skills-spec]: [Agent Skills Specification](https://agentskills.io/specification)。

