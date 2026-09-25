# Third-party notices

`src/design/input.rs` の TextElement の layout / painting は、GPUI 0.2.2 の
`examples/input.rs` を参照・改変しています。

- Copyright 2022–2025 Zed Industries, Inc.
- Apache License 2.0: [license text](licenses/GPUI-APACHE-2.0.txt)
- Source: https://docs.rs/crate/gpui/0.2.2/source/examples/input.rs
- 変更: Unicode buffer の分離、IME 選択範囲の修正、横スクロール、focus、送信、配色。

直接依存は `Cargo.toml` で完全固定し、推移的依存は `Cargo.lock` で固定します。
各依存の package metadata 上の license は以下のとおりです。

| Crate | Version | License |
| --- | --- | --- |
| gpui | 0.2.2 | Apache-2.0 |
| async-channel | 2.5.0 | Apache-2.0 OR MIT |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |

GPUI の対象は公開 crate です。Zed アプリケーション全体を組み込んでいません。
配布物を作る段階では、対象 platform の全依存・同梱物の NOTICE を別途集約します。
