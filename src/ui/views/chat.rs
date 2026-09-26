use super::*;
use gpui_kit::component::{
    message::{Message as KitMessage, MessageContent, MessageHeader},
    message_scroller::MessageScroller,
};

impl Workspace {
    pub(super) fn conversation(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.sessions[self.selected];
        if session.model.chat.is_empty() {
            let p = ds::theme(cx);
            return div().size_full().flex().flex_col().justify_end().p(px(space::XL)).gap_4()
                .child(div().size(px(52.)).rounded(px(12.)).bg(rgb(p.accent_soft)).text_color(rgb(p.accent_text))
                    .flex().items_center().justify_center().text_size(px(32.)).child("#"))
                .child(div().text_size(px(24.)).font_weight(FontWeight::SEMIBOLD).child("ここから、作業をはじめましょう"))
                .child(div().text_color(rgb(p.secondary)).line_height(px(22.)).child(format!("{} の新しいセッションです。依頼と返答はこのチャンネルに、ツールやサブエージェントの実行はスレッドにまとまります。", self.workspace_name)))
                .child(div().flex().flex_wrap().gap_2().children([
                    ("調査する", "このプロジェクトの構成と、改善できる点を調べてください。"),
                    ("変更をレビュー", "現在の変更をレビューして、問題点を説明してください。"),
                ].into_iter().enumerate().map(|(i, (label, prompt))| Button::new(("channel-starter", i), label)
                    .control_size(ControlSize::Small)
                    .on_click(cx.listener(move |this, _, window, cx| this.fill_prompt(prompt, window, cx))))))
                .child(
                    Button::icon("empty-run", Icon::Pencil, "依頼を入力").on_click(cx.listener(
                        |this, _, window, cx| {
                            window
                                .focus(&this.sessions[this.selected].composer.focus_handle(cx), cx);
                        },
                    )),
                )
                .into_any_element();
        }
        let weak = cx.weak_entity();
        let session_id = session.model.id.clone();
        MessageScroller::new(
            "conversation",
            session.chat_list.clone(),
            move |row, _, cx| {
                let Some(view) = weak.upgrade() else {
                    return div().into_any_element();
                };
                let workspace = view.read(cx);
                let Some(session) = workspace.sessions.iter().find(|s| s.model.id == session_id)
                else {
                    return div().into_any_element();
                };
                let Some(block) = session.model.chat.get(row) else {
                    return div().into_any_element();
                };
                let p = ds::theme(cx);
                let is_thread_root = block.speaker == Speaker::User
                    && row
                        .checked_sub(1)
                        .and_then(|i| session.model.chat.get(i))
                        .is_none_or(|previous| previous.message_id != block.message_id);
                let thread = block
                    .thread_id
                    .and_then(|id| session.model.threads.iter().find(|thread| thread.id == id));
                let copy = block.text.clone();
                let callback_view = weak.clone();
                let copy_button = Button::icon(("copy-chat", row), Icon::Copy, "この部分をコピー")
                    .control_size(ControlSize::Small)
                    .on_click(move |_, _, cx| {
                        let _ = callback_view.update(cx, |this, cx| {
                            this.copy(copy.clone(), "本文をコピーしました", cx)
                        });
                    });
                let (name, initial, tone) = match block.speaker {
                    Speaker::User => ("あなた", "You", Tone::Neutral),
                    Speaker::Assistant => (
                        if session.uses_openai_icon() {
                            "Codex"
                        } else {
                            "エージェント"
                        },
                        "",
                        Tone::Accent,
                    ),
                    Speaker::Notice => ("状態の更新", "", Tone::Neutral),
                };
                let body = if block.speaker == Speaker::Notice {
                    div()
                        .pl(px(44.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::Info.view(p.muted))
                        .child(caption(block.text.clone(), cx).flex_1().min_w_0())
                        .child(copy_button)
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .items_start()
                        .gap_3()
                        .child(div().pt_1().child(if block.speaker == Speaker::Assistant {
                            session.agent_avatar(cx)
                        } else {
                            ds::avatar(initial, tone, cx)
                        }))
                        .child(
                            KitMessage::new()
                                .w_full()
                                .header(
                                    MessageHeader::new()
                                        .content_inset(false)
                                        .w_full()
                                        .text_size(px(13.))
                                        .text_color(rgb(p.text))
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(name),
                                                )
                                                .when(
                                                    block.speaker == Speaker::Assistant
                                                        && !session.uses_workspace(),
                                                    |v| {
                                                        v.child(ds::badge(
                                                            "疑似応答",
                                                            Tone::Neutral,
                                                            cx,
                                                        ))
                                                    },
                                                ),
                                        )
                                        .child(copy_button),
                                )
                                .content(
                                    MessageContent::new()
                                        .w_full()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::NORMAL)
                                        .line_height(px(22.))
                                        .text_color(rgb(p.text))
                                        .child(block.text.clone()),
                                ),
                        )
                        .into_any_element()
                };
                div()
                    .w_full()
                    .px(px(space::XL))
                    .py(px(space::SM))
                    .child(div().w_full().max_w(px(840.)).mx_auto().child(body).when(
                        is_thread_root,
                        |v| {
                            v.when_some(thread, |v, thread| {
                                let target = weak.clone();
                                let id = thread.id;
                                let count = thread.activity_count();
                                v.child(
                                    div().pl(px(44.)).pt_2().child(
                                        Button::new(
                                            ("message-thread", row),
                                            if count == 0 {
                                                "実行スレッドを開く".into()
                                            } else {
                                                format!("{count} 件の実行 · スレッドを開く")
                                            },
                                        )
                                        .with_icon(Icon::MessageSquare)
                                        .variant(ButtonVariant::Ghost)
                                        .control_size(ControlSize::Small)
                                        .text_color(rgb(p.accent_text))
                                        .on_click(
                                            move |_, window, cx| {
                                                let _ = target.update(cx, |this, cx| {
                                                    this.open_thread(id, window, cx)
                                                });
                                            },
                                        ),
                                    ),
                                )
                            })
                        },
                    ))
                    .into_any_element()
            },
        )
        .with_jump_button_label("最新のメッセージへ")
        .size_full()
        .into_any_element()
    }
}
