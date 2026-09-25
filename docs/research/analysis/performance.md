---
type: Research Analysis
title: "Rust + GPUIで高速性を成立させる設計"
description: "UI、context、tool、model、保存の遅延を分解し、測定可能な性能目標を提案する。"
tags: [research, agent-harness, analysis]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: gpui-profile
    resource: "../products/zed.md"
    title: "Zed・GPUI分析"
  - id: aider-profile
    resource: "../products/aider.md"
    title: "Aider分析"
  - id: cline-profile
    resource: "../products/cline.md"
    title: "Cline分析"
  - id: tokio-channels
    resource: "https://tokio.rs/tokio/tutorial/channels"
    title: "Tokio Channels"
  - id: ghostty
    resource: "https://ghostty.org/docs/about"
    title: "About Ghostty"
---

# 高速性の設計

**ここに記載する数値はすべて提案目標で、実測結果ではない。** 使用言語や描画方式だけではtask完了速度を比較できない。GPUIの設計、repo map、checkpoint I/Oという各製品の違いを測定軸へ分ける。[^gpui-profile][^aider-profile][^cline-profile]
terminalの速度も起動、scroll、I/O、frame等の複数の意味を持つというGhosttyの説明が参考になる。[^ghostty]

## 所要時間の分解

直列部分のtask時間は、おおむね次の区間から成る。

```text
UI入力 → context選択 → provider送信 → 最初の応答 → 生成
       → tool準備/承認 → 実行 → 結果整理 → 次の推論 → 検証 → review
```

並列処理がある場合、全区間の単純合計ではなくcritical pathを測る。fast UI、fast model、少ないturn、早いtool、少ない人の手戻りは別の改善である。

| 遅延・負荷 | 計測点 | 主な改善案 |
| --- | --- | --- |
| cold start | process起動→入力可能 | 索引・plugin起動を遅延、sessionをページ単位でload |
| 入力応答 | input event→描画present | 長い処理をUIから外す、再描画範囲を狭める |
| context準備 | user送信→model request | 差分索引、結果cache、必要fileだけ読む |
| 初回応答 | request送信→最初のdelta | provider/model/networkを別タグで記録 |
| tool周辺負担 | tool提案→process開始 | schema cache、不要なprocess起動を避ける |
| tool本体 | 開始→終了 | 独立readの並列化、適切なtimeout |
| 保存 | event生成→durable境界 | 小さいtransaction、blob退避、blocking worker |
| レビュー | 完了提示→利用者の採否 | diff・test・成果物を対応付ける |

## GPUIの表示設計

長い会話・log・diffは可視範囲を中心に描画する。Markdownをtokenごとに全文parseせず、確定blockと生成中tailを分ける。可変行高の測定cacheを持ち、上側の内容が更新されてもscroll位置が飛ばないようにする。

コード差分はfile単位、hunk単位で遅延loadする。巨大file、長い一行、binary、改行コード差を通常の短いcode表示と同じ経路へ無条件に流さない。

日本語IME、結合文字、絵文字、幅の異なるglyph、選択・copyを性能試験にも含める。描画が速くても入力中の文字が壊れるなら受入不可である。

## streamingとbackpressure

bounded channelを使い、遅いUIが無制限のmessageをメモリーへ蓄積しないようにする。Tokioの容量付きchannelは送受の速度差を制御する部品になる。[^tokio-channels]

- token deltaは短い間隔でbatch化する。最初は16〜33msを候補とし実測で調整する。
- lifecycle、承認、終了、errorは合流・破棄しない。
- 大きいtool出力はdiskへ連続保存し、UIには範囲と参照だけを渡す。
- 単一の巨大Stringへの連続copyや、全履歴cloneを避ける。
- CPU負荷の高いdiff、構文解析、圧縮をI/O用executorやUI threadで占有しない。

networkの受信を止めることがprovider timeoutを招く場合は、描画queueより前でdisk spoolする。queueの上限に達した時の方針を決めておく。

## 初期の性能目標案

参照機は暫定的にApple Silicon、16GB RAM、内蔵SSD、1920×1080相当60Hzのdesktopを想定する。実際のCPU/OS/GPUを初回測定時に固定して記録する。以下はmodel/networkをstubにしたUI・core単体の目標。

| 指標 | 提案目標 | 条件 |
| --- | --- | --- |
| 入力可能まで | warm p95 ≤ 500ms、cold p95 ≤ 1.5s | 索引完了は待たない |
| 入力→表示 | p95 ≤ 50ms | 生成logを受信しながらIME入力 |
| scroll時frame | p95 ≤ 16.7ms | 60Hz、10万event、可視範囲描画 |
| delta表示待ち | p95 ≤ 50ms | backend受信後→表示。model時間は除外 |
| cancel要求送出 | p95 ≤ 100ms | UI→対象runtimeへの要求 |
| 制御可能なfixture停止 | p95 ≤ 1s | subprocess treeを含む。停止確認まで |
| GUI/core idle RSS | ≤ 250MiBを仮目標 | LSP、外部agent、local modelは別欄にも計上 |
| 100MiB log受信 | 全文RAM保持なし、増加が設計上限内 | spoolと可視cacheを測る |

他OSも同じ数値を自動的に満たすとはしない。macOSで初期基準を作り、Linux/Windowsは別の基準機で検証する。外部agent等の別processを除外して「小さいアプリ」と宣伝せず、process tree合計も必ず記録する。

## 成果を悪化させる高速化

contextを削りすぎて再調査が増える、承認を省いて誤操作が増える、小さいモデルへのroutingで手戻りが増える、checkpointを省いて復旧できなくなる場合、局所的な短縮がtask全体を遅くする。

採否は成功taskあたりの時間・費用・レビュー負担で判断する。[評価計画](evaluation.md)に品質と速度を同時に測る手順を定義した。

[^gpui-profile]: [Zed・GPUI分析](../products/zed.md)。
[^aider-profile]: [Aider分析](../products/aider.md)。
[^cline-profile]: [Cline分析](../products/cline.md)。
[^tokio-channels]: [Tokio Channels](https://tokio.rs/tokio/tutorial/channels)。
[^ghostty]: [About Ghostty](https://ghostty.org/docs/about)。

