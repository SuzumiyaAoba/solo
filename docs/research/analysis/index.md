# 横断比較と設計

* [ハーネス・エージェント・エディター・SDKの責任分界](taxonomy.md) - 比較対象を同じ階層で評価するための用語と設計軸を整理する。
* [製品横断比較と選定シナリオ](comparison.md) - 35比較単位の機能上の中心、統合負担、適した参照段階と機構の差を比較する。
* [ACP・MCP・Skills・AGENTS.md・A2A・LSP](protocols.md) - 相互運用の役割、バージョン差、Rust実装候補、能力交渉を比較する。
* [Rust + GPUIの推奨アーキテクチャ](architecture.md) - UI・自作harness・外部agent・executor・永続化を分離する設計提案。
* [コンテキスト・圧縮・メモリーの設計](context-memory.md) - 検索、指示、tool出力、長期知識を分け、予算と鮮度を管理する方法を比較する。
* [権限・隔離・復旧・人の介入](security-recovery.md) - 許可、OS境界、変更追跡、クラッシュ後の副作用を別々に設計する。
* [Rust + GPUIで高速性を成立させる設計](performance.md) - UI、context、tool、model、保存の遅延を分解し、測定可能な性能目標を提案する。
* [比較実験・性能評価・受入条件](evaluation.md) - 同条件比較、失敗注入、task品質、費用と操作性の測定計画を定義する。
* [再利用候補・依存・ライセンス・費用・継続性](reuse.md) - 直接依存、外部プロセス、設計参照を分け、更新・費用・移行の負担を比較する。
* [周辺エディター・terminalから学ぶ設計](adjacent-tools.md) - Neovim、Helix、WezTerm、Ghosttyをハーネスとは別のUI・言語・PTYの参照として比較する。
* [実装ロードマップと未解決事項](roadmap.md) - 単一agentの正しさとUI性能を先に検証し、並列化やremoteを段階導入する。
