# CLI・terminalを中心に使うハーネス

* [Codex — CLI・App Server・SDK](codex.md) - 会話・ターン・実行項目を分離した外部ハーネス接続と承認イベントの参照例。
* [Claude Code・Claude Agent SDK](claude-code.md) - ツール・権限・文脈管理を統合したハーネスとSDK化の参照例。
* [Gemini CLI](gemini-cli.md) - Google系CLIの拡張・隔離・ACP対応を比較する参照例。
* [OpenCode](opencode.md) - HTTPサーバーと複数UI、モデル選択、権限制御を分離した参照例。
* [Pi Coding Agent](pi.md) - 最小のコア、ツリー状セッション、RPC、拡張による機能追加の参照例。
* [Aider](aider.md) - リポジトリ地図、編集モデル分業、Git連携によるコード変更の参照例。
* [goose](goose.md) - Rust製の実行基盤、MCP拡張、再利用可能なrecipeの参照例。
* [Crush](crush.md) - Go製TUIとLSP・MCP・複数モデルを組み合わせた対話操作の参照例。
* [Warp](warp.md) - 自然言語とshell操作を連続させるterminal中心のUXの参照例。

# エディター・GUIを中心に使う製品群

* [Zed・GPUI](zed.md) - Rust製エディターとACPホスト、GPU描画UIの最重要参照例。
* [Cursor — Editor・CLI・Cloud Agents](cursor.md) - 編集・検索・ブラウザー検証・非同期クラウド作業を統合したUXの参照例。
* [Windsurf / Devin Desktop・Cascade](devin-desktop.md) - コードと会話のモード、編集状況の文脈化、checkpointの参照例。
* [GitHub Copilot — VS Code・CLI・Cloud Agent](copilot.md) - エディター内作業、terminal、IssueからPRまでの委任を比較する製品群。
* [Cline](cline.md) - 利用者の承認、checkpoint、複数UIとSDKを持つagent coreの参照例。
* [Roo Code — 提供終了を含む歴史的参照](roo-code.md) - 役割別モードとオーケストレーション、および製品終了への備えを学ぶ事例。
* [Kilo Code](kilo.md) - IDE・CLI・モデルgateway・自動化を横断する開発プラットフォームの参照例。
* [Continue](continue.md) - モデル・ルール・ツールの構成可能性とローカルモデル運用の参照例。
* [Kiro](kiro.md) - 仕様・設計・タスクを永続成果物にする開発工程と複数UI共通ハーネスの参照例。
* [JetBrains Junie](junie.md) - IDEのコード理解、計画と実装のモデル分離、途中介入を重視する参照例。
* [Google Antigravity](antigravity.md) - エージェント管理画面、IDE・CLI・SDKの共通ハーネス化を学ぶ参照例。
* [Augment Code・Auggie CLI](augment.md) - 大規模コード文脈の索引化とIDE・terminal・自動化の連携を比較する事例。

# 常駐型・複数チャネルのエージェント

* [OpenClaw](openclaw.md) - 常駐Gateway、複数チャネル、セッション分離、再接続を扱う汎用エージェント基盤。

# クラウド・非同期委任・アプリ生成

* [Amp](amp.md) - モデルルーティング、共有thread、ローカルとクラウド作業の連続性を学ぶ参照例。
* [OpenHands — Agent Canvas・SDK・Agent Server](openhands.md) - コード作業向けagent、UI、サーバー、sandboxを分離する実行基盤の参照例。
* [Devin](devin.md) - 非同期の開発委任とIDE・shell・browserへの利用者介入を学ぶ参照例。
* [Replit Agent・Bolt・Lovable](app-builders.md) - アプリ生成・プレビュー・公開を一体化する製品群から成果物中心のUXを学ぶ。

# SDK・実行基盤・研究用ハーネス

* [OpenAI Agents SDK・Agents API](openai-agents.md) - 自分で運用するagent loopとmanaged harnessの責任分界を比較する事例。
* [LangGraph・Deep Agents](langgraph-deepagents.md) - 永続実行runtimeと高機能harnessを層として分ける参照例。
* [Google Agent Development Kit](google-adk.md) - agent、決定的workflow、評価、配備を分離する汎用基盤の参照例。
* [Microsoft Agent Framework・AutoGen・Semantic Kernel](microsoft-agent-framework.md) - agent抽象、harness、workflowを統合する現行基盤と旧系譜の整理。
* [Rig](rig.md) - Rustでprovider・tool・agent状態を扱うための直接的な部品候補。
* [Pydantic AI・Pydantic AI Harness](pydantic-ai.md) - 型付きtool・出力検証、durable実行、組合せ可能なharness機能の参照例。
* [SWE-agent・mini-SWE-agent](swe-agents.md) - 最小実行ループ、環境分離、再現可能な評価を学ぶ研究用基盤。
* [CrewAI](crewai.md) - 決定的なFlowと役割を持つagent群を分けるオーケストレーションの参照例。
* [Letta](letta.md) - 永続メモリーと実行ハーネスの世代差、長期利用の設計を学ぶ参照例。
