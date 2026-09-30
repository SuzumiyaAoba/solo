//! 起動引数の解釈。`run()` から分離し、フラグの優先順位とビルド条件を集約する。
use std::path::PathBuf;

/// 起動引数から解釈した実行モード。
pub(super) enum CliMode {
    /// --help: 使い方を表示して終了
    Help,
    /// --visual DIR: PNG 出力して終了（要 feature gui-visual）
    Visual(
        /// feature 無しではパスを読まずエラー表示だけ行う。
        #[cfg_attr(not(feature = "gui-visual"), allow(dead_code))]
        PathBuf,
    ),
    /// 通常のアプリ起動
    App(AppOptions),
    /// 現在のビルドでは使えないフラグ。メッセージを表示して終了する。
    /// debug ビルドでは --smoke が常に受理されるため生成されない。
    #[cfg_attr(debug_assertions, allow(dead_code))]
    Unsupported(&'static str),
}

pub(super) struct AppOptions {
    /// --smoke: 実画面の検証後に終了（debug ビルドのみ有効）
    pub(super) smoke: bool,
    /// --light: ライトテーマで起動
    pub(super) light: bool,
    /// --compact: 小さいウィンドウで起動
    pub(super) compact: bool,
}

pub(super) const USAGE: &str = "Solo\n  --light     ライトテーマで起動\n  --compact   小さいウィンドウで起動\n  --smoke     実画面の検証後に終了\n  --visual D  PNG で実描画を出力して終了（要 feature gui-visual）";

/// CLI 引数を実行モードに変換する。未知の引数は従来どおり無視する。
pub(super) fn parse(args: &[String]) -> CliMode {
    if args.iter().any(|arg| arg == "--help") {
        return CliMode::Help;
    }
    let visual = args
        .iter()
        .position(|arg| arg == "--visual")
        .and_then(|i| args.get(i + 1).map(PathBuf::from))
        .or_else(|| {
            args.iter()
                .find_map(|arg| arg.strip_prefix("--visual=").map(PathBuf::from))
        });
    if let Some(dir) = visual {
        return CliMode::Visual(dir);
    }
    let smoke = args.iter().any(|arg| arg == "--smoke");
    #[cfg(not(debug_assertions))]
    let smoke = {
        if smoke {
            return CliMode::Unsupported("--smoke は debug ビルドでのみ有効です");
        }
        false
    };
    CliMode::App(AppOptions {
        smoke,
        light: args.iter().any(|arg| arg == "--light"),
        compact: args.iter().any(|arg| arg == "--compact"),
    })
}
