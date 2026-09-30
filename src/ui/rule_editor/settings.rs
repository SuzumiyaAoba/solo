use super::CommandRuleEditor;
use gpui_kit::component::WindowExt;
use gpui_kit::{AnyElement, Context, FontWeight, div, prelude::*, px, rgb};
use solo::{
    config::ApprovalMode,
    design::{self as ds, Button, ButtonVariant, Tone, typography},
};

impl CommandRuleEditor {
    /// 承認モードページ。Manual / Bypass / Auto と Auto 判定モデルを保存する。
    pub(super) fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        let description = match self.approval.mode {
            ApprovalMode::Manual => "リストで決まらない要求を、実行前に確認します。",
            ApprovalMode::Bypass => {
                "承認確認と Allow / Deny の判定を省略し、すべての要求を許可します。"
            }
            ApprovalMode::Auto => {
                "Deny / Allow を優先し、未登録の要求を指定モデルで判定します。判断できない場合は手動確認に戻ります。"
            }
        };
        div()
            .id("approval-settings-form")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                ds::card(cx)
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("承認モード · 全プロジェクト共通"),
                    )
                    .child(self.mode_picker.clone())
                    .child(
                        div()
                            .text_size(px(typography::LABEL))
                            .text_color(rgb(p.secondary))
                            .child(description),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Auto の判定モデル"),
                    )
                    .child(self.auto_model.clone())
                    .child(
                        div()
                            .text_size(px(typography::CAPTION))
                            .text_color(rgb(p.muted))
                            .child(format!(
                                "ChatGPT ログインで利用できるモデル名 · 制限時間 {} 秒",
                                self.approval.auto.timeout_seconds,
                            )),
                    )
                    .child(
                        Button::new("save-approval-settings", "承認設定を保存")
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_settings(cx);
                                if this.error.is_none() {
                                    window.push_notification("承認設定を保存しました", cx);
                                }
                            })),
                    ),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(ds::alert("設定を保存できません", error, Tone::Danger, cx))
            })
            .when_some(self.store.as_ref().ok(), |v, store| {
                v.child(
                    div()
                        .text_size(px(typography::CAPTION))
                        .text_color(rgb(p.muted))
                        .child(format!("設定ファイル: {}", store.path().display())),
                )
            })
            .into_any_element()
    }
}
