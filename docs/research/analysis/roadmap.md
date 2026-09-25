---
type: Research Analysis
title: "実装ロードマップと未解決事項"
description: "単一agentの正しさとUI性能を先に検証し、並列化やremoteを段階導入する。"
tags: [research, agent-harness, analysis]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: architecture
    resource: "architecture.md"
    title: "アーキテクチャ"
  - id: comparison
    resource: "comparison.md"
    title: "横断比較"
  - id: evaluation
    resource: "evaluation.md"
    title: "評価計画"
  - id: reuse
    resource: "reuse.md"
    title: "再利用方針"
---

# 実装ロードマップ

以下は調査から導く提案であり、この変更でアプリケーションを実装したものではない。Rust + GPUIという方針を前提に、実行を自分で所有するcoreと外部backendを混同しない順序を採る。[^architecture][^comparison]

## 段階と終了条件

| 段階 | 作るもの | 次へ進む条件 |
| --- | --- | --- |
| 0. 表示と契約の試作 | GPUI会話、diff、log、疑似event、session一覧、IME | 長いstreamでも入力が止まらず、未知eventや切断を表示できる |
| 1. 単一agent core | 一provider、read/search/edit/exec、policy、予算、stop理由 | 固定taskを完了し、拒否操作・競合を実行しない |
| 2. 永続化と復旧 | event/artefact保存、再開、snapshot、export | crash/電断相当の失敗注入で、履歴と副作用の不明状態を保てる |
| 3. 外部接続の比較 | ACP clientまたはCodex等のadapter、一MCP経路 | native coreとの同一画面表示と機能差表示、契約試験に成功 |
| 4. 文脈と操作の改善 | LSP、repo map、段階的skills、圧縮、plan文書 | baselineより成功率/時間/費用が改善、制約を失わない |
| 5. 複数作業・remote | task依存、必要時worktree、remote executor | 競合・予算・停止・環境消失を制御できる |
| 6. 長期運用 | 検証可能なmemory、plugin配布、更新・移行 | scope、訂正、失効、互換性、exportを維持 |

表示の試作は既存harnessを使って早めてもよいが、段階1の独自loopができるまでは「自作harness完成」としない。各終了条件の具体的なtaskは[評価計画](evaluation.md)。[^evaluation]

## 初期版の機能境界

必須は単一workspace、単一agent、streaming、file/commandの証跡、差分確認、途中指示・cancel、権限、予算、再開である。

後回しにする候補は、全言語対応editor、一般的なworkflow言語、agentの大規模並列化、vector DB必須化、marketplace、hosting内蔵、複数cloudの一括対応。不要だと結論したのではなく、先に品質とコストを測れるcoreを作るための順序である。

## 実装前の短い技術検証

| 課題 | 検証内容 | 判断 |
| --- | --- | --- |
| GPUI APIとplatform | 日本語IME、font、scroll、window消滅後のtask | 対象OSを順次広げるか |
| provider層 | Rigと最小直接実装で必要なstream/usageを処理 | library採用か独自adapterか |
| 外部harness | 一つだけ接続し、承認・cancel・再開を通す | ACPか独自APIを最初に使うか |
| PTY | interactive commandとchild treeを制御 | pipeとの使い分けと部品採用 |
| 保存 | event＋blobの途中で落とし、整合を検査 | DB・transaction境界の確定 |
| patch | dirty fileと未保存bufferの競合 | edit toolの契約を確定 |
| context | 字句検索とrepo mapのbaseline | embedding導入の必要性 |

候補部品と版固定の扱いは[再利用方針](reuse.md)。[^reuse]

## まだ決めていないこと

OSの対応順、完全offlineの要否、最初のprovider、subscription連携の要否、同時task数、pluginの実行形式、remote実行先、内蔵editorの範囲は利用目的によって変わる。この調査ではmacOSを初期性能基準の候補にしたが、利用者の対応OS要件を確定したわけではない。

coreの独立性と記録形式を先に固めれば、これらを後から変えても全面的な作り直しを減らせる。

## 追加調査の優先順位

1. GPUIと直接採用crateのrelease/commit、依存licenseを固定。
2. 最初に使うmodelとharnessで、tool calling、context長、認証の契約を確認。
3. OS別sandboxの実効範囲を小さいfixtureで測定。
4. 価格とrate limitを選んだ契約で確認し、予算計算へ反映。
5. 外部protocolの対応版とadapter経由の機能差を記録。
6. 初期の成功taskを蓄積してから、parallel/memoryの追加効果を測定。

プロダクトの差別化は、機能数より「待たないUI」「実行が見える」「途中で直せる」「壊れても状況を失わない」という利用体験に置く案を提案する。

[^architecture]: [アーキテクチャ](architecture.md)。
[^comparison]: [横断比較](comparison.md)。
[^evaluation]: [評価計画](evaluation.md)。
[^reuse]: [再利用方針](reuse.md)。

