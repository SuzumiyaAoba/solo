---
type: Research Analysis
title: "調査方法・範囲・OKF v0.2形式"
description: "調査時点、証拠の扱い、比較単位、OKFの採用仕様と検証範囲を定義する。"
tags: [research, agent-harness]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: okf-spec
    resource: "https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md"
    title: "Open Knowledge Format Specification v0.2"
  - id: profiles
    resource: "products/index.md"
    title: "製品別分析の索引"
---

# 調査方法・範囲

## 調査の目的と時点

Rust + GPUIで自作する、コード作業を中心としたハーネスの設計材料を集める。2026-09-25時点で取得できた公式ドキュメント・公式リポジトリを根拠とする。製品を購入・実行しての比較、内部コード全体の監査、独立した性能測定は行っていない。

[製品別分析](products/index.md)は35の比較単位。単位は製品ファミリーまたは密接な基盤群であり、35の独立製品を同条件でbenchmarkしたという意味ではない。CLI・editor・cloudの入口が異なっても同じ製品群は原則まとめた。[^profiles]

対象選定は代表的な利用形態、公開資料の充実度、Rust/GPUIへの設計上の関連を基準とした。市場占有率の順位や、全製品の網羅を証明する調査ではない。大手製品に加えPi、mini-SWE-agent、Rig等の設計上重要な例を含む。

## 事実・分析・提案の区別

| 表記・場所 | 意味 |
| --- | --- |
| 確認できた機能と構造 | 調査時に一次資料で読めた仕様。稼働検証済みという意味ではない |
| 長所・短所の分析 | 確認した構造からの著者の評価・推論。実測値ではない |
| Rust + GPUIへの示唆 | このプロジェクト向けの設計提案 |
| 未確認 / 要確認 | 文書で足りない、取得できない、経路やversion差が未解決 |
| 実験 / preview | 公式資料がその状態として説明したもの |
| 歴史的参照 | 終了告知や後継推奨を確認したもの |

記載が見つからない機能を「非対応」とはしない。model support、BYOK、subscription、tool support、native sandbox、remote sandbox、checkpointはそれぞれ別に扱う。ベンダーの「高速」「最高性能」は独立評価として転載しない。

数値ランキング、星取表の恣意的な総合点、モデル・予算の異なるSWE-bench値の横並びは採用しない。短所には「公式に明示された制約」と「構造から予想される負担」があるため、各profileの文脈で区別する。

## 出典の記録と限界

出典は各conceptの `sources` と、その `id` に対応する脚注で記録する。横断分析は製品profileを内部sourceとして参照し、そこから外部一次資料へ辿れる。

参照日は取得日であって、文書の公開日・更新日ではない。確認していない `last_modified`、利用数、version、commit SHAは作らない。URLはcanonicalな移行先を優先したが、全ページの不変snapshotを保存したわけではない。厳密な将来の再現には採用候補のrelease/commit固定と資料保存が必要である。

[OpenAI SDK](products/openai-agents.md)では検索結果と取得本文の不一致を確認した。本文で再確認できない保守方針は未確定として残す。リンク先が存在することと、比較表の全項目を裏付けることも区別する。

## 「OKF v2」の解釈と採用仕様

依頼の「OKF v2」は、確認できた公式の第2版 **Open Knowledge Format v0.2** と解釈した。公式仕様には `Version 0.2` とあり、major version 2.0という宣言は使用しない。仕様の `main` を2026-09-25に確認した。[^okf-spec]

このbundleの構成は次の通り。

- rootの `index.md` のみ `okf_version: "0.2"` を宣言する。subdirectoryのindexはfrontmatterを持たない。
- index/log以外のMarkdownはconceptで、非空の `type` とYAML frontmatterを持つ。
- concept IDはbundle内pathから `.md` を除いたもの。本文の相対Markdown linkで関連を表す。
- `generated` は作成過程、`sources` は出典を示す。人による確認は実施していないため `verified` を付けない。
- `status: draft` はレビュー前を意味する。`research` はこのbundle独自の拡張metadata。
- 時刻はUTC offset付き。`stale_after` は2026-10-25 00:00 JSTを再確認の目安とする編集方針で、そこまでは仕様が不変という保証ではない。
- 実測値を生成していないため、通常のResearch Analysis等を使い、Attested Computationを名乗らない。

以上は公式のbundle、出典・信頼、index/log、versionの規則に基づく。[^okf-spec]

## 検証の範囲

[ローカル検査器](tools/validate.rb)はYAML、必須metadata、日時、index/log構造、ローカルリンク、source IDと脚注、索引の到達性を検査する。このbundle向けの追加品質検査であり、汎用の公式OKF認証器ではない。機械検査が通っても内容の事実性を人が承認したことにはならない。

結果は [validation.json](validation.json)。製品の性能、URLの将来の可用性、SDK互換性はこの検査の対象外である。

[^okf-spec]: [Open Knowledge Format Specification v0.2](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md)。
[^profiles]: [製品別分析の索引](products/index.md)。
