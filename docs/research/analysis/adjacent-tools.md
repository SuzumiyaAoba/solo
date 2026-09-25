---
type: Research Analysis
title: "周辺エディター・terminalから学ぶ設計"
description: "Neovim、Helix、WezTerm、Ghosttyをハーネスとは別のUI・言語・PTYの参照として比較する。"
tags: [research, agent-harness]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: neovim
    resource: "https://neovim.io/"
    title: "Neovim official overview"
  - id: helix
    resource: "https://helix-editor.com/"
    title: "Helix official overview"
  - id: wezterm
    resource: "https://wezterm.org/index.html"
    title: "WezTerm official overview"
  - id: ghostty
    resource: "https://ghostty.org/docs/about"
    title: "About Ghostty"
  - id: portable-pty
    resource: "https://docs.rs/portable-pty/latest/portable_pty/"
    title: "portable-pty API documentation"
---

# 周辺エディターとterminal

この4製品は35のagent製品・基盤グループとは別の補助比較。単体で同じagent loopを提供する製品としては数えない。

## 機能と設計評価

| 対象 | 確認できた特徴 | 長所の分析 | 短所・条件の分析 | 自作への示唆 |
| --- | --- | --- | --- | --- |
| Neovim | RPC、embed、Lua拡張、LSP、Tree-sitter | editor coreとUIを分けて拡張できる | runtimeとpluginの状態を含む統合が必要 | フルeditor自作と外部editor接続を比較 |
| Helix | Rust、terminal、構造を使う編集、Tree-sitter | codeの構造と選択操作を結びつける | terminal向け操作をGUIへそのまま移せない | 構文上の単位でdiff/選択を扱う |
| WezTerm | Rust製cross-platform terminalとmultiplexer | pane・PTY・remoteを一貫して考えられる | font、制御列、platform差の実装範囲が広い | terminal engineと描画UIの責任を分ける |
| Ghostty | platform共通coreとnative frontend | 入力・platform適合をUIごとに考えられる | libghosttyの安定性は採用版で再確認が必要 | 単なるlog viewと本物のterminalを分ける |

特徴の根拠は公式概要。長所・短所と示唆は本レポートの分析。[^neovim][^helix][^wezterm][^ghostty]

## terminalを作るときの境界

PTYはprocessとのI/Oであり、terminal emulatorは制御列を解釈して画面状態を作るもの。log viewerはその一部しか扱わない。portable-ptyは前者のAPIであって、全terminal描画が完成するlibraryではない。[^portable-pty]

エージェントのshell toolを最初に実装するときは、非対話commandをpipeで処理し、必要な対話操作だけPTYを使う選択肢もある。いずれもexit code、stdin、resize、cancel、process groupを明示する。

## editorの範囲を限定する

初期版はread-only code view、diff、fileを外部editorで開く機能から始める案が合理的。高度な補完、debugger、全言語refactor、modal editing、巨大plugin市場を同時に再現する必要はない。

独自editorを内蔵する場合は、未保存buffer、診断version、undo履歴、agentのpatch適用を一つのworkspaceモデルに統合する。この一貫性がないと、UIが速くても利用者の編集を壊す。

関連: [Zed](../products/zed.md)、[Warp](../products/warp.md)、[性能](performance.md)。

[^neovim]: [Neovim official overview](https://neovim.io/)。
[^helix]: [Helix official overview](https://helix-editor.com/)。
[^wezterm]: [WezTerm official overview](https://wezterm.org/index.html)。
[^ghostty]: [About Ghostty](https://ghostty.org/docs/about)。
[^portable-pty]: [portable-pty API documentation](https://docs.rs/portable-pty/latest/portable_pty/)。
