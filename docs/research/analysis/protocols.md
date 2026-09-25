---
type: Research Analysis
title: "ACP・MCP・Skills・AGENTS.md・A2A・LSP"
description: "相互運用の役割、バージョン差、Rust実装候補、能力交渉を比較する。"
tags: [research, agent-harness, analysis]
status: draft
generated:
  by: process:codex-research
  at: "2026-09-25T06:09:34Z"
stale_after: "2026-10-25T00:00:00+09:00"
sources:
  - id: acp-spec
    resource: "https://agentclientprotocol.com/protocol/v1/overview"
    title: "ACP protocol v1 overview"
  - id: acp-rust
    resource: "https://github.com/agentclientprotocol/rust-sdk"
    title: "Official ACP Rust SDK"
  - id: mcp-current
    resource: "https://modelcontextprotocol.io/specification/2026-07-28/architecture"
    title: "MCP architecture 2026-07-28"
  - id: mcp-previous
    resource: "https://modelcontextprotocol.io/specification/2025-11-25/architecture"
    title: "MCP architecture 2025-11-25"
  - id: mcp-rust
    resource: "https://github.com/modelcontextprotocol/rust-sdk"
    title: "Official MCP Rust SDK"
  - id: skills-spec
    resource: "https://agentskills.io/specification"
    title: "Agent Skills Specification"
  - id: agentsmd
    resource: "https://agents.md/"
    title: "AGENTS.md"
  - id: a2a
    resource: "https://a2a-protocol.org/latest/topics/what-is-a2a/"
    title: "What is A2A?"
  - id: lsp
    resource: "https://github.com/microsoft/language-server-protocol"
    title: "Language Server Protocol repository"
---

# 相互運用の境界

## どこを接続するものか

| 仕様・方式 | 接続対象 / 役割 | 自作での優先度 | 代わりにはならないもの |
| --- | --- | --- | --- |
| ACP | editor/client ↔ coding agent | 高: 外部harness接続 | モデルprovider API、OS sandbox |
| MCP | hostのclient ↔ tool/resource/prompt server | 高: 外部tool接続 | agent全体の会話・task管理 |
| Agent Skills | 手順・script・参照資料のpackage | 中: 手順再利用 | 強制的なpermission |
| AGENTS.md | repositoryでの作業指示 | 高: 既存projectへの適合 | 認証、実行隔離 |
| LSP | editor ↔ language server | 中: 診断・コード情報 | coding agent loop |
| A2A | 独立したagent同士の協調 | 後期: 組織やサービスを跨ぐ委任 | local tool実行やeditor protocol |
| 独自API | Codex App Server、Pi RPC、OpenCode HTTP等 | 高: 比較用adapter | 他社protocolとの自動互換 |

ACP/MCP/Skills/AGENTS.md/A2A/LSPの役割はそれぞれの一次資料に基づく。優先度は本プロジェクト向けの判断。[^acp-spec][^mcp-current][^skills-spec][^agentsmd][^a2a][^lsp]

## ACPの扱い

v1はJSON-RPC 2.0、初期化、session作成、prompt、更新通知、permission要求、cancel等を持つ。履歴load等は能力による。Rust SDKは公式に存在するが、repositoryにはstable v1とdraft v2・unstable featureの区別がある。[^acp-spec][^acp-rust]

したがって、名前だけの「ACP対応」では不十分。以下をadapterごとに保存する提案とする。

- protocol版とagent実体のversion。
- native対応か、変換adapter経由か。
- sessionの一覧/再開/分岐、model切替、画像、terminal、file操作、承認の対応範囲。
- cancelの到達保証と、停止完了を確認する方法。
- agentが直接host filesystemへアクセスするか、client経由で操作するか。

Zedがあるagentを接続できることは、そのagent本体が直接ACPを実装している証拠とは限らない。変換層を比較対象に含める。

## MCPの版を固定する

調査時のlatestは2026-07-28へredirectした。そのarchitectureは要求ごとのversion/capabilityとdiscoveryを説明する。一方、2025-11-25資料はstateful sessionと初期化時の能力交換を説明しており、単に「MCPは常にこのhandshake」と実装してはいけない。[^mcp-current][^mcp-previous]

Rust側の候補は公式SDK。採用releaseがどのspec/transportを実装するかを別途確認する。新しいspecの存在からcrateの実装済みを推測しない。[^mcp-rust]

初期実装は対応版とstdio等のtransportを限定し、接続matrixをテストする。remote認証、再接続、tool一覧変更、巨大な結果は独立の受入項目にする。tool名はserverごとにnamespace化し、同名toolで承認が流用されないようにする。

## Skillsと指示ファイル

Skillsはmetadata→本体→必要なresourceという段階的な読込みを想定する。すべての手順を毎回system promptへ詰める必要はない。[^skills-spec]
AGENTS.mdはagent用のproject指示。自作側でscopeと読み込んだpathを示し、CLAUDE.md/GEMINI.md等との互換機能は優先順位を定義して追加する。[^agentsmd]

手順にscriptが含まれていても自動的に実行を許可しない。skillの内容はtool permissionより低い境界に置く。repositoryに追加された新しいMCP設定やpluginは、明示されたtrust設定を通して有効化する。

## 自作の永続形式へ外部仕様を漏らさない

内部は `Session / Turn / ToolInvocation / Approval / Artifact / Usage` を基準にする。各protocolのIDと生イベントは別に保存する。未知のfieldは保持し、UIで理解しない能力を勝手に有効化しない。

「共通の最小機能だけに丸める」だけでは高度な能力を失う。共通core＋能力で有効化する拡張領域とし、落ちた情報や再現できない操作を表示する。

関連: [アーキテクチャ](architecture.md)、[再利用](reuse.md)。

[^acp-spec]: [ACP protocol v1 overview](https://agentclientprotocol.com/protocol/v1/overview)。
[^acp-rust]: [Official ACP Rust SDK](https://github.com/agentclientprotocol/rust-sdk)。
[^mcp-current]: [MCP architecture 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/architecture)。
[^mcp-previous]: [MCP architecture 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/architecture)。
[^mcp-rust]: [Official MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)。
[^skills-spec]: [Agent Skills Specification](https://agentskills.io/specification)。
[^agentsmd]: [AGENTS.md](https://agents.md/)。
[^a2a]: [What is A2A?](https://a2a-protocol.org/latest/topics/what-is-a2a/)。
[^lsp]: [Language Server Protocol repository](https://github.com/microsoft/language-server-protocol)。

