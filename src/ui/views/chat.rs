use super::*;
use gpui_kit::component::{
    highlighter::HighlightTheme,
    message::{Message as KitMessage, MessageContent, MessageFooter, MessageHeader},
    message_scroller::MessageScroller,
    text::{SelectionFormat, TextView, TextViewStyle},
};
use solo::projection::ChatBlock;
use std::{collections::VecDeque, ops::Range};

/// この行数を超える応答は先頭だけを描き、「すべて表示」で全文に戻す。
const COLLAPSE_LINES: usize = 60;
/// 1 バブルへ流し込む表示テキストの上限。巨大な 1 メッセージ(数十万 byte の
/// ダンプ等)が毎フレーム全文レイアウトされないよう、先頭だけを描く。
/// 展開時は全文を渡す。コピーは表示と独立に全文連結する。
const MAX_RENDER_BYTES: usize = 64 * 1024;

/// `row` が (speaker, message_id) の連続ブロックの先頭か。
/// 先頭だけがバブルを描き、継続行は高さ 0 で隠す。
pub(in crate::ui) fn is_run_head(chat: &VecDeque<ChatBlock>, row: usize) -> bool {
    let Some(block) = chat.get(row) else {
        return false;
    };
    row.checked_sub(1)
        .and_then(|i| chat.get(i))
        .is_none_or(|prev| prev.message_id != block.message_id || prev.speaker != block.speaker)
}

/// `row` を含む (speaker, message_id) ランの範囲 [start, end)。
/// 追記で伸びたブロックがランの途中なら、全文を描く先頭行から測り直すために使う。
pub(in crate::ui) fn run_bounds(chat: &VecDeque<ChatBlock>, row: usize) -> Option<Range<usize>> {
    let first = chat.get(row)?;
    let mut start = row;
    while start > 0 {
        let prev = &chat[start - 1];
        if prev.message_id == first.message_id && prev.speaker == first.speaker {
            start -= 1;
        } else {
            break;
        }
    }
    let mut end = row + 1;
    while end < chat.len() {
        let next = &chat[end];
        if next.message_id == first.message_id && next.speaker == first.speaker {
            end += 1;
        } else {
            break;
        }
    }
    Some(start..end)
}

/// ラン先頭 `row` から、同じ (speaker, message_id) のブロックを連結した全文とブロック数。
/// ブロックは byte 境界で割れただけなので、区切り文字は挟まない。
pub(in crate::ui) fn message_text(chat: &VecDeque<ChatBlock>, row: usize) -> (String, usize) {
    let Some(first) = chat.get(row) else {
        return (String::new(), 0);
    };
    let mut text = first.text.clone();
    let mut count = 1;
    for block in chat.iter().skip(row + 1) {
        if block.message_id != first.message_id || block.speaker != first.speaker {
            break;
        }
        text.push_str(&block.text);
        count += 1;
    }
    (text, count)
}

/// 末尾から遡って最初に見つかった User メッセージの全文。composer の
/// 「↑で直前の依頼を呼び出す」用。ラン先頭から連結するため、ブロック分割
/// された依頼でも本文全体が戻る。User メッセージが無ければ None。
pub(in crate::ui) fn last_user_prompt(chat: &VecDeque<ChatBlock>) -> Option<String> {
    let row = chat
        .iter()
        .rposition(|block| block.speaker == Speaker::User)?;
    let head = run_bounds(chat, row).map_or(row, |bounds| bounds.start);
    Some(message_text(chat, head).0)
}

/// ラン先頭 `row` の表示用テキストを `max` byte まで連結する。
/// ランが `max` を超えた場合は `truncated = true` を返し、文字境界で切る。
/// 巨大メッセージでもラン先頭行ごとのコストが `max` に収まる。
fn head_text(chat: &VecDeque<ChatBlock>, row: usize, max: usize) -> (String, bool) {
    let Some(first) = chat.get(row) else {
        return (String::new(), false);
    };
    let mut text = String::new();
    for block in chat.iter().skip(row) {
        if block.message_id != first.message_id || block.speaker != first.speaker {
            break;
        }
        let rest = max.saturating_sub(text.len());
        if block.text.len() > rest {
            text.push_str(&block.text[..block.text.floor_char_boundary(rest)]);
            return (text, true);
        }
        text.push_str(&block.text);
    }
    (text, false)
}

/// assistant 応答の本文。Markdown を ds のパレットと等幅フォントに合わせて描く。
/// element id は message_id ベースにする。行番号は破棄・スプラす前後で変わるため使わない。
fn assistant_markdown(
    session_id: &SessionId,
    message_id: &str,
    text: &str,
    max_lines: Option<usize>,
    cx: &App,
) -> TextView {
    let p = ds::theme(cx);
    let dark = ds::scheme(cx) == ColorScheme::Dark;
    let code_block = div()
        .bg(ds::glass(p.canvas, 0.6))
        .border_1()
        .border_color(glass(p.border, 0.7))
        .rounded(px(ds::radius::CONTROL))
        .px_3()
        .py_2()
        .font_family(typography::MONO)
        .style()
        .clone();
    let mut table = StyleRefinement::default();
    table.overflow.x = Some(Overflow::Scroll);
    let table_head = StyleRefinement {
        background: Some(ds::glass(p.hover, ds::GLASS_SURFACE).into()),
        ..Default::default()
    };
    let mut style = TextViewStyle::default()
        // 本文の 22px 行間より狭い段落間隔にし、連続する見出し・段落を詰める。
        .paragraph_gap(rems(0.5))
        .heading_font_size(|level, _| {
            px(if level <= 2 {
                typography::HEADING
            } else {
                typography::LEAD
            })
        })
        .code_block(code_block)
        .inline_code(HighlightStyle {
            background_color: Some(ds::glass(p.hover, ds::GLASS_HOVER).into()),
            ..Default::default()
        })
        .table(table)
        .table_head(table_head);
    style.highlight_theme = if dark {
        HighlightTheme::default_dark()
    } else {
        HighlightTheme::default_light()
    };
    let view = TextView::markdown(
        ElementId::Name(SharedString::from(format!("md-{session_id}-{message_id}"))),
        text.to_owned(),
    )
    .selectable(true)
    .selection_format(SelectionFormat::Source)
    .stream_fade(true)
    .style(style)
    .code_block_actions(|block, _, _cx| {
        let code = block.code().to_string();
        Button::icon("copy-code", Icon::Copy, "コードをコピー")
            .control_size(ControlSize::Small)
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
            })
    })
    .on_link_click(|url, _, _, cx| {
        if url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:") {
            cx.open_url(url);
        }
    });
    match max_lines {
        Some(max) => view.max_lines(max),
        None => view,
    }
}

impl Workspace {
    pub(super) fn conversation(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = self.session();
        if session.model.chat().is_empty() {
            let p = ds::theme(cx);
            return div().size_full().flex().flex_col().justify_end().p(px(space::XL)).gap_4()
                .child(div().size(px(52.)).rounded(px(ds::radius::DIALOG)).bg(rgb(p.accent_soft)).text_color(rgb(p.accent_text))
                    .flex().items_center().justify_center().text_size(px(typography::TITLE)).child("#"))
                .child(div().text_size(px(typography::HEADING)).font_weight(FontWeight::SEMIBOLD).child("ここから、作業をはじめましょう"))
                .child(div().text_color(rgb(p.secondary)).line_height(px(20.)).child(format!("{} の新しいセッションです。依頼と返答はこのチャンネルに、ツールやサブエージェントの実行はスレッドにまとまります。", self.workspace_name)))
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
                                .focus(&this.session().composer.focus_handle(cx), cx);
                        },
                    )),
                )
                .into_any_element();
        }
        let weak = cx.weak_entity();
        let session_id = session.model.id.clone();
        // 行ラッパー側の行間(pb_8)を消し、メッセージ間の余白はバブル側で付ける。
        // 継続ブロックの行は高さ 0 になるため、余白が残ると宙に浮いた空白になる。
        let mut row_style = StyleRefinement::default();
        row_style.padding.bottom = Some(px(0.).into());
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
                let chat = session.model.chat();
                let Some(block) = chat.get(row) else {
                    return div().into_any_element();
                };
                // 継続ブロックは先頭行が全文を描くので、行だけ残して高さを消す。
                if !is_run_head(chat, row) {
                    return div().h_0().overflow_hidden().into_any_element();
                }
                let expanded = block.speaker == Speaker::Assistant
                    && session.view.expanded_messages.contains(&block.message_id);
                // 描画は先頭 `MAX_RENDER_BYTES` まで。巨大ランでも行ごとの
                // レイアウト費が読み込みブロック数に比例しないようにする。
                let (text, truncated) = head_text(
                    chat,
                    row,
                    if expanded {
                        usize::MAX
                    } else {
                        MAX_RENDER_BYTES
                    },
                );
                let run_len = run_bounds(chat, row).map_or(1, |r| r.end - r.start);
                // ランの後にも行が続くときだけ、次のメッセージとの間隔を空ける。
                let more_follow = row + run_len < chat.len();
                let capped_lines = text.matches('\n').count() + 1;
                let shown = if truncated {
                    format!("{text}\n\n…")
                } else {
                    text
                };
                let p = ds::theme(cx);
                let is_thread_root = block.speaker == Speaker::User;
                let thread = block.thread_id.and_then(|id| {
                    session
                        .model
                        .threads()
                        .iter()
                        .find(|thread| thread.id == id)
                });
                let copy_id = block.message_id.clone();
                let copy_session = session_id.clone();
                let callback_view = weak.clone();
                let copy_button = Button::icon(("copy-chat", row), Icon::Copy, "この部分をコピー")
                    .control_size(ControlSize::Small)
                    .on_click(move |_, _, cx| {
                        let _ = callback_view.update(cx, |this, cx| {
                            let Some(index) = this.session_index(&copy_session) else {
                                return;
                            };
                            // 表示は先頭で切っているので、コピーはランの全文を
                            // クリック時に連結し直す。
                            let text = {
                                let chat = this.session_at(index).model.chat();
                                chat.iter()
                                    .position(|b| b.message_id == copy_id)
                                    .map(|head| message_text(chat, head).0)
                            };
                            if let Some(text) = text {
                                this.copy(text, "本文をコピーしました", cx);
                            }
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
                        .pl(px(space::SECTION))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::Info.view(p.muted))
                        .child(caption(shown.clone(), cx).flex_1().min_w_0())
                        .child(copy_button)
                        .into_any_element()
                } else {
                    let content = MessageContent::new()
                        .w_full()
                        .text_size(px(typography::LEAD))
                        .font_weight(FontWeight::NORMAL)
                        .line_height(px(22.))
                        .text_color(rgb(p.text));
                    // assistant だけ Markdown。user の先頭の '#' が見出し化しないよう、
                    // user/notice はこれまで通りプレーンテキストのままにする。
                    let (content, footer) = if block.speaker == Speaker::Assistant {
                        let collapsible = truncated || capped_lines > COLLAPSE_LINES;
                        let line_count = if truncated {
                            // 表示は先頭だけなので、ボタンラベルの行数は
                            // ラン全体から数え直す。
                            chat.iter()
                                .skip(row)
                                .take(run_len)
                                .map(|b| b.text.matches('\n').count())
                                .sum::<usize>()
                                + 1
                        } else {
                            capped_lines
                        };
                        let content = content.child(assistant_markdown(
                            &session_id,
                            &block.message_id,
                            &shown,
                            if collapsible && !expanded {
                                Some(COLLAPSE_LINES)
                            } else {
                                None
                            },
                            cx,
                        ));
                        let footer = collapsible.then(|| {
                            let toggle_id = block.message_id.clone();
                            let toggle_session = session_id.clone();
                            let toggle_view = weak.clone();
                            MessageFooter::new().content_inset(false).child(
                                Button::new(
                                    ("message-expand", row),
                                    if expanded {
                                        "折りたたむ".into()
                                    } else {
                                        format!("すべて表示（{line_count} 行）")
                                    },
                                )
                                .variant(ButtonVariant::Ghost)
                                .control_size(ControlSize::Small)
                                .text_color(rgb(p.accent_text))
                                .on_click(move |_, _, cx| {
                                    let _ = toggle_view.update(cx, |this, cx| {
                                        let Some(index) = this.session_index(&toggle_session)
                                        else {
                                            return;
                                        };
                                        let session = this.session_at_mut(index);
                                        if !session.view.expanded_messages.remove(&toggle_id) {
                                            session
                                                .view
                                                .expanded_messages
                                                .insert(toggle_id.clone());
                                        }
                                        // 高さが変わるランだけ再測定する。
                                        let range = session
                                            .model
                                            .chat()
                                            .iter()
                                            .position(|b| b.message_id == toggle_id)
                                            .and_then(|pos| run_bounds(session.model.chat(), pos));
                                        if let Some(range) = range {
                                            session.chat_list.update(cx, |list, cx| {
                                                list.remeasure_items(range, cx);
                                            });
                                        }
                                        cx.notify();
                                    });
                                }),
                            )
                        });
                        (content, footer)
                    } else {
                        (content.child(shown.clone()), None)
                    };
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
                                        .text_size(px(typography::BODY))
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
                                .content(content)
                                .when_some(footer, |message, footer| message.footer(footer)),
                        )
                        .into_any_element()
                };
                div()
                    .w_full()
                    .px(px(space::XL))
                    .pt(px(space::SM))
                    .pb(px(if more_follow { space::XXL } else { space::SM }))
                    .child(div().w_full().max_w(px(840.)).mx_auto().child(body).when(
                        is_thread_root,
                        |v| {
                            v.when_some(thread, |v, thread| {
                                let target = weak.clone();
                                let id = thread.id;
                                let count = thread.activity_count();
                                v.child(
                                    div().pl(px(space::SECTION)).pt_2().child(
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
        .with_row_style(row_style)
        .with_jump_button_label("最新のメッセージへ")
        .size_full()
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{is_run_head, message_text, run_bounds};
    use solo::projection::{ChatBlock, Speaker};
    use std::collections::VecDeque;

    fn block(speaker: Speaker, id: &str, text: &str) -> ChatBlock {
        ChatBlock {
            speaker,
            message_id: id.into(),
            text: text.into(),
            thread_id: None,
        }
    }

    #[test]
    fn consecutive_blocks_of_one_message_form_a_single_run() {
        let mut chat = VecDeque::new();
        chat.push_back(block(Speaker::User, "prompt-1", "質問"));
        chat.push_back(block(Speaker::Assistant, "a", "前半 "));
        chat.push_back(block(Speaker::Assistant, "a", "後半"));
        chat.push_back(block(Speaker::Notice, "end-1", "完了"));

        assert!(is_run_head(&chat, 0));
        assert!(is_run_head(&chat, 1));
        assert!(!is_run_head(&chat, 2));
        assert!(is_run_head(&chat, 3));
        assert_eq!(message_text(&chat, 1), ("前半 後半".to_owned(), 2));
        assert_eq!(run_bounds(&chat, 1), Some(1..3));
        assert_eq!(run_bounds(&chat, 2), Some(1..3));
    }

    #[test]
    fn same_message_id_split_by_another_speaker_starts_a_new_run() {
        let mut chat = VecDeque::new();
        chat.push_back(block(Speaker::Assistant, "a", "最初"));
        chat.push_back(block(Speaker::Notice, "n", "中断"));
        chat.push_back(block(Speaker::Assistant, "a", "再開"));

        assert!(is_run_head(&chat, 2));
        assert_eq!(message_text(&chat, 0), ("最初".to_owned(), 1));
        assert_eq!(message_text(&chat, 2), ("再開".to_owned(), 1));
        assert_eq!(run_bounds(&chat, 2), Some(2..3));
    }

    #[test]
    fn speaker_change_breaks_the_run_even_with_the_same_id() {
        let mut chat = VecDeque::new();
        chat.push_back(block(Speaker::Assistant, "m", "応答"));
        chat.push_back(block(Speaker::User, "m", "同 id の別話者"));

        assert!(is_run_head(&chat, 0));
        assert!(is_run_head(&chat, 1));
        assert_eq!(message_text(&chat, 0), ("応答".to_owned(), 1));
    }

    #[test]
    fn empty_or_out_of_range_rows_are_safe() {
        let chat = VecDeque::new();
        assert!(!is_run_head(&chat, 0));
        assert_eq!(message_text(&chat, 0), (String::new(), 0));
        assert_eq!(run_bounds(&chat, 0), None);
    }
}
