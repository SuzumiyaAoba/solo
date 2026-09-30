use super::CommandRuleEditor;
use gpui_kit::{AnyElement, Context, FontWeight, div, prelude::*, px, rgb};
use solo::{
    command_rules::Decision,
    design::{self as ds, Tone, typography},
    harness::workspace::{is_read_only_tool, tool_description, tool_names},
};

impl CommandRuleEditor {
    /// ツールページ。基本ツールごとの Allow / Deny / Ask を設定する。
    pub(super) fn tools_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = ds::theme(cx);
        div()
            .id("tool-rules")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                ds::card(cx)
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("基本ツール · このプロジェクト"),
                    )
                    .child(
                        div()
                            .text_size(px(typography::LABEL))
                            .text_color(rgb(p.secondary))
                            .child("Allow は確認を省略し、Deny は常に拒否、Ask はコマンドルールと同じ承認確認に戻します。既定はツールごとの安全な初期値です。"),
                    )
                    .children(tool_names().enumerate().map(|(index, name)| {
                        let decision = self.rules.tool_decision(name);
                        let default = if is_read_only_tool(name) { "許可" } else { "確認" };
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(56.))
                                    .flex_shrink_0()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(typography::LABEL))
                                    .text_color(rgb(p.secondary))
                                    .truncate()
                                    .child(format!("{default} · {}", tool_description(name))),
                            )
                            .child(
                                div()
                                    .w(px(148.))
                                    .flex_shrink_0()
                                    .child(self.tool_pickers[index].clone()),
                            )
                            .when_some(decision, |v, decision| {
                                v.child(ds::badge(
                                    match decision {
                                        Decision::Allow => "許可",
                                        Decision::Deny => "拒否",
                                        Decision::Ask => "確認",
                                    },
                                    match decision {
                                        Decision::Allow => Tone::Success,
                                        Decision::Deny => Tone::Danger,
                                        Decision::Ask => Tone::Neutral,
                                    },
                                    cx,
                                ))
                            })
                    })),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(ds::alert("ツール設定を保存できません", error, Tone::Danger, cx))
            })
            .into_any_element()
    }
}
