# Third-party notices

UI は [GPUI Kit](https://gpui-kit.com/) 0.6.6 を使用しています。
Kit が対応する `gpui-pre` 0.3.6、gpui-base、gpui-component と assets をまとめて提供します。
旧 GPUI input example を基にした独自 TextElement は廃止し、入力処理も Kit に委ねています。

- GPUI Kit / GPUI: Apache-2.0 ([license text](licenses/GPUI-APACHE-2.0.txt))
- Source: https://github.com/longbridge/gpui-kit/tree/v0.6.6
- Lucide icons: ISC。Feather 由来のアイコンは MIT。
  [同梱ライセンス](licenses/LUCIDE-LICENSE.txt) / [公式出典](https://lucide.dev/license)

直接依存は `Cargo.toml` で完全固定し、推移的依存は `Cargo.lock` で固定します。
各依存の package metadata 上の license は以下のとおりです。

| Crate | Version | License |
| --- | --- | --- |
| gpui-kit | 0.6.6 | Apache-2.0 |
| async-channel | 2.5.0 | Apache-2.0 OR MIT |
| futures | 0.3.34 | MIT OR Apache-2.0 |
| genai-agentprism | 0.7.0-beta.19.1-agentprism | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| tokio | 1.53.1 | MIT |
| tokio-util | 0.7.19 | MIT |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |

GPUI の対象は公開 crate です。Zed アプリケーション全体を組み込んでいません。
配布物を作る段階では、対象 platform の全依存・同梱物の NOTICE を別途集約します。
