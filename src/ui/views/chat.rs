use super::*;
use gpui_kit::component::{
    message::{Message as KitMessage, MessageContent, MessageHeader},
    message_scroller::MessageScroller,
};

impl Workspace {
    pub(super) fn conversation(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.sessions[self.selected];
        if session.model.chat.is_empty() {
            return empty(Icon::MessageSquare, "会話なし", cx)
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
                    ds::alert(name, block.text.clone(), tone, cx)
                        .child(copy_button)
                        .into_any_element()
                } else {
                    KitMessage::new()
                        .w_full()
                        .avatar(if block.speaker == Speaker::Assistant {
                            session.agent_avatar(cx)
                        } else {
                            ds::avatar(initial, tone, cx)
                        })
                        .header(
                            MessageHeader::new()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().font_weight(FontWeight::MEDIUM).child(name))
                                        .when(
                                            block.speaker == Speaker::Assistant
                                                && !session.uses_workspace(),
                                            |v| v.child(ds::badge("疑似応答", Tone::Neutral, cx)),
                                        ),
                                )
                                .child(copy_button),
                        )
                        .content(
                            MessageContent::new()
                                .w_full()
                                .text_size(px(typography::LEAD))
                                .line_height(px(typography::LEAD + space::SM))
                                .text_color(rgb(p.text))
                                .child(block.text.clone()),
                        )
                        .into_any_element()
                };
                div()
                    .w_full()
                    .px(px(space::XL))
                    .py(px(space::SM))
                    .child(div().w_full().max_w(px(840.)).mx_auto().child(body))
                    .into_any_element()
            },
        )
        .with_jump_button_label("最新のメッセージへ")
        .size_full()
        .into_any_element()
    }
}
