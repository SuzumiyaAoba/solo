use super::*;

impl Gallery {
    pub(super) fn overview(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme(cx);
        stack()
            .child(row().gap_2().child(badge("GPUI Kit 0.6.6", Tone::Neutral, cx)).child(badge("Light & Dark", Tone::Neutral, cx)).child(badge("Keyboard ready", Tone::Neutral, cx)))
            .child(div().flex().gap_5()
                .child(card(cx).flex_1().min_w_0().p_6().gap_5()
                    .child(div().flex().items_center().justify_between().child(label("COMPONENT COMPOSITION", cx)).child(Icon::Layers.view(p.muted)))
                    .child(div().flex().flex_col().gap_2().child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child("次のアイデアを、ここから。"))
                        .child(div().text_size(px(12.)).line_height(px(21.)).text_color(rgb(p.secondary)).child("部品を組み合わせた、小さなワークスペース。\n入力や操作を試して、使い心地を確かめてください。")))
                    .child(field("Workspace name", self.fields[0].clone(), "日本語・絵文字もそのまま入力できます。", cx))
                    .child(divider(cx))
                    .child(div().flex().items_center().justify_between().gap_3()
                        .child(Toggle::switch("overview-auto-save", "自動保存", self.switch_on).on_change(cx.listener(|this, value: &bool, _, cx| { this.switch_on = *value; cx.notify(); })))
                        .child(Button::new("overview-create", "変更を保存").variant(ButtonVariant::Primary).with_icon(Icon::Check).loading(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.start_loading(cx))))))
                .child(card(cx).w(px(280.)).flex_shrink_0().p_5().gap_5()
                    .child(div().flex().items_center().justify_between().child(div().font_weight(FontWeight::MEDIUM).child("Activity")).child(badge("3 updates", Tone::Neutral, cx)))
                    .children([
                        ("S", "コンポーネントを整える", "Foundations · Just now", Tone::Accent),
                        ("D", "両テーマを確認する", "Appearance · 2 min ago", Tone::Success),
                        ("U", "入力の使い心地を磨く", "Interaction · 5 min ago", Tone::Neutral),
                    ].into_iter().map(|(initial, title, detail, tone)| {
                        div().flex().gap_3().child(avatar(initial, tone, cx))
                            .child(div().flex().flex_col().gap_1().child(div().text_size(px(12.)).child(title)).child(label(detail, cx)))
                    }))
                    .child(divider(cx))
                    .child(div().flex().flex_col().gap_2().child(div().flex().justify_between().child(label("Design coverage", cx)).child(label("8 categories", cx))).child(progress(1., Tone::Accent, cx)))
                    .child(Button::new("overview-explore", "コンポーネントを見る").trailing_icon(Icon::ArrowRight).variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Buttons, cx))))))
            .child(section("ひとつの体系で、画面をつくる。", "メインアプリと同じコンポーネントを、そのまま操作できます。", cx)
                .child(div().grid().grid_cols(3).gap_3().children([
                    (Page::Foundations, "色・文字・余白・アイコン"), (Page::Buttons, "優先順位と操作状態"), (Page::Inputs, "入力・検証・読み取り専用"),
                    (Page::Selection, "選択・切替・ドロップダウン"), (Page::Feedback, "状態・進捗・空の画面"), (Page::Overlays, "通知・確認・フォーカス"),
                ].into_iter().map(|(page, description)| {
                    card(cx).p_4().gap_2()
                        .child(Button::new(("overview-link", page as usize), page.title()).variant(ButtonVariant::Ghost).with_icon(page.icon()).justify_start().w_full()
                            .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx))))
                        .child(label(description, cx))
                }))))
            .child(div().flex().items_center().gap_3().pt_1().child(Icon::Info.view(p.muted)).child(label("控えめな境界線、4px 基準の余白、意味を持つ色。細部まで同じルールで。", cx)))
            .into_any_element()
    }

    pub(super) fn foundations(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme(cx);
        stack()
            .child(
                section(
                    "Color",
                    "色名ではなく、画面での役割を基準に選ぶ。クリックで HEX をコピーできます。",
                    cx,
                )
                .child(
                    div().grid().grid_cols(4).gap_3().children(
                        [
                            ("Canvas", p.canvas),
                            ("Surface", p.surface),
                            ("Elevated", p.elevated),
                            ("Border", p.border),
                            ("Text", p.text),
                            ("Secondary", p.secondary),
                            ("Muted", p.muted),
                            ("Accent", p.accent),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (name, color))| {
                            let hex = format!("#{color:06X}");
                            card(cx)
                                .p_3()
                                .gap_3()
                                .child(
                                    div()
                                        .h(px(42.))
                                        .rounded(px(5.))
                                        .bg(rgb(color))
                                        .border_1()
                                        .border_color(rgb(p.border)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(div().text_size(px(12.)).child(name))
                                        .child(
                                            Button::new(("copy-color", i), hex.clone())
                                                .control_size(ControlSize::Small)
                                                .variant(ButtonVariant::Ghost)
                                                .font_family(typography::MONO)
                                                .text_size(px(10.))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(hex.clone()),
                                                    );
                                                    this.message(
                                                        format!("{hex} をコピーしました"),
                                                        Tone::Success,
                                                        cx,
                                                    );
                                                })),
                                        ),
                                )
                        }),
                    ),
                )
                .child(
                    row()
                        .child(badge(
                            format!("Text / Canvas {:.1}:1", contrast(p.text, p.canvas)),
                            Tone::Neutral,
                            cx,
                        ))
                        .child(badge(
                            format!(
                                "Secondary / Surface {:.1}:1",
                                contrast(p.secondary, p.surface)
                            ),
                            Tone::Neutral,
                            cx,
                        ))
                        .child(badge(
                            format!("On accent {:.1}:1", contrast(p.on_accent, p.accent)),
                            Tone::Neutral,
                            cx,
                        )),
                ),
            )
            .child(
                section(
                    "Typography",
                    "システムフォントを使い、日本語と英数字の読みやすさを揃える。",
                    cx,
                )
                .child(
                    card(cx).p_5().gap_5().children(
                        [
                            (
                                "Title",
                                typography::TITLE,
                                "考えることに、集中する。",
                                FontWeight::SEMIBOLD,
                            ),
                            (
                                "Heading",
                                typography::HEADING,
                                "A place for focused work.",
                                FontWeight::SEMIBOLD,
                            ),
                            (
                                "Body",
                                typography::BODY,
                                "小さな一貫性が、画面全体の使い心地をつくります。",
                                FontWeight::NORMAL,
                            ),
                            (
                                "Caption",
                                typography::CAPTION,
                                "Updated just now · 表示を確認できます",
                                FontWeight::NORMAL,
                            ),
                        ]
                        .into_iter()
                        .map(|(name, size, example, weight)| {
                            div()
                                .flex()
                                .items_center()
                                .gap_5()
                                .child(
                                    div()
                                        .w(px(100.))
                                        .flex_shrink_0()
                                        .text_size(px(11.))
                                        .text_color(rgb(p.muted))
                                        .child(format!("{name} / {size:.0}")),
                                )
                                .child(div().text_size(px(size)).font_weight(weight).child(example))
                        }),
                    ),
                ),
            )
            .child(
                section(
                    "Spacing & radius",
                    "余白は4px基準。角丸は用途ごとに4 / 6 / 10 / 12px。",
                    cx,
                )
                .child(
                    card(cx)
                        .p_5()
                        .gap_5()
                        .child(
                            row()
                                .items_end()
                                .gap_5()
                                .children(space::SCALE.into_iter().map(|n| {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .w(px(34.))
                                                .h(px(n))
                                                .rounded(px(2.))
                                                .bg(rgb(p.accent)),
                                        )
                                        .child(label(format!("{n:.0}px"), cx))
                                })),
                        )
                        .child(divider(cx))
                        .child(
                            row().gap_5().children(
                                [radius::SMALL, radius::CONTROL, radius::CARD, radius::DIALOG]
                                    .into_iter()
                                    .map(|r| {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(70.))
                                                    .h(px(42.))
                                                    .border_1()
                                                    .border_color(rgb(p.control_border))
                                                    .rounded(px(r))
                                                    .bg(rgb(p.elevated)),
                                            )
                                            .child(label(format!("{r:.0}px"), cx))
                                    }),
                            ),
                        ),
                ),
            )
            .child(
                section(
                    "Iconography",
                    "Lucide の UI アイコンと Lobe Icons のブランドアイコン。アイコンだけの操作には名前を添える。",
                    cx,
                )
                .child(card(cx).p_4().child(row().gap_2().children(
                    Icon::ALL.into_iter().enumerate().map(|(i, icon)| {
                        Button::icon(("icon-sample", i), icon, icon.name()).on_click(cx.listener(
                            move |this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    icon.name().into(),
                                ));
                                this.message(
                                    format!("{} をコピーしました", icon.name()),
                                    Tone::Success,
                                    cx,
                                );
                            },
                        ))
                    }),
                ))),
            )
            .into_any_element()
    }

    pub(super) fn buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        stack()
            .child(section("Hierarchy", "Primary は主操作にひとつ。補助操作には Secondary、Ghost を使います。", cx)
                .child(example("Variants", cx).child(row()
                    .child(Button::new("primary-probe", "新しいセッション").variant(ButtonVariant::Primary).with_icon(Icon::Plus)
                        .on_click(cx.listener(|this, _, _, cx| this.record_click(cx))))
                    .child(Button::new("secondary-example", "変更を保存").with_icon(Icon::Check).on_click(cx.listener(|this, _, _, cx| this.record_click(cx))))
                    .child(Button::new("ghost-example", "詳細を見る").variant(ButtonVariant::Ghost).trailing_icon(Icon::ArrowRight).on_click(cx.listener(|this, _, _, cx| this.record_click(cx))))
                    .child(Button::new("danger-example", "削除").variant(ButtonVariant::Danger).with_icon(Icon::Trash).on_click(cx.listener(|this, _, _, cx| this.message("Danger ボタンを操作しました", Tone::Danger, cx))))))
                .child(label(format!("操作回数: {} · Enter / Space でも実行できます", self.clicks), cx)))
            .child(section("Size", "高さと文字サイズを一緒に変え、どの密度でも比率を保つ。", cx)
                .child(example("Small / Medium / Large", cx).child(row().items_end().children([ControlSize::Small, ControlSize::Medium, ControlSize::Large].into_iter().enumerate().map(|(i, size)| {
                    div().flex().flex_col().gap_3().child(label(format!("{:.0} px", size.height()), cx))
                        .child(Button::new(("button-size", i), "セッションを作成").control_size(size).with_icon(Icon::Plus).on_click(cx.listener(|this, _, _, cx| this.record_click(cx))))
                })))))
            .child(section("Interaction states", "hover・押下・focus の比較用表示です。実際のマウスとキーボードでも同じ状態になります。", cx)
                .child(card(cx).p_5().child(row().gap_5().items_end().children([
                    ("Default", PreviewState::Rest), ("Hover", PreviewState::Hover), ("Pressed", PreviewState::Pressed), ("Focus", PreviewState::Focus),
                ].into_iter().enumerate().map(|(i, (name, state))| {
                    div().flex().flex_col().gap_3().child(label(name, cx)).child(Button::new(("state", i), "ボタン").preview(state).on_click(cx.listener(|this, _, _, cx| this.record_click(cx))))
                }))
                    .child(div().flex().flex_col().gap_3().child(label("Disabled", cx)).child(Button::new("disabled-probe", "ボタン").disabled(true)
                        .on_click(cx.listener(|this, _, _, cx| { this.record_click(cx); }))))
                    .child(div().flex().flex_col().gap_3().child(label("Loading", cx)).child(Button::new("loading-preview", "保存中…").variant(ButtonVariant::Primary).loading(true))))))
            .child(section("Icon buttons & loading", "名前は tooltip で確認できます。保存ボタンは読み込み中の多重実行を防ぎます。", cx)
                .child(example("Try it", cx).child(row()
                    .children([(Icon::Plus, "追加"), (Icon::Search, "検索"), (Icon::Copy, "コピー"), (Icon::Settings, "設定"), (Icon::Trash, "削除")].into_iter().enumerate().map(|(i, (icon, name))| {
                        Button::icon(("icon-button", i), icon, name).on_click(cx.listener(|this, _, _, cx| this.record_click(cx)))
                    }))
                    .child(div().w_3())
                    .child(Button::new("async-button", if self.loading { "保存中…" } else { "保存を試す" }).variant(ButtonVariant::Primary).loading(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.start_loading(cx)))))))
            .into_any_element()
    }

    pub(super) fn inputs(&self, cx: &mut Context<Self>) -> AnyElement {
        let invalid = self.fields[2].read(cx).invalid;
        stack()
            .child(
                section(
                    "Text input",
                    "ラベルと補助説明で入力の目的を示す。⌘ Enter で入力内容を確定できます。",
                    cx,
                )
                .child(
                    card(cx)
                        .p_5()
                        .gap_5()
                        .child(
                            row()
                                .items_start()
                                .child(field(
                                    "プロジェクト名",
                                    self.fields[0].clone(),
                                    "IME 変換中は送信されません。",
                                    cx,
                                ))
                                .child(field(
                                    "検索",
                                    self.fields[1].clone(),
                                    "アイコンを添えた検索フィールド。",
                                    cx,
                                )),
                        )
                        .child(divider(cx))
                        .child(
                            row()
                                .items_start()
                                .child(field(
                                    "入力の検証",
                                    self.fields[2].clone(),
                                    if invalid {
                                        "3文字以上で入力してください。"
                                    } else {
                                        "利用できる名前です。"
                                    },
                                    cx,
                                ))
                                .child(field(
                                    "Disabled",
                                    self.fields[3].clone(),
                                    "Tab 移動・文字入力・貼付けの対象になりません。",
                                    cx,
                                )),
                        )
                        .child(divider(cx))
                        .child(field(
                            "Read only",
                            self.fields[4].clone(),
                            "選択とコピーができます。変更はできません。",
                            cx,
                        )),
                ),
            )
            .child(
                section(
                    "Size",
                    "ボタンと揃えた28 / 34 / 40px。横幅が足りない長文はキャレットを追従します。",
                    cx,
                )
                .child(
                    card(cx).p_5().gap_4().child(
                        row()
                            .items_start()
                            .child(field(
                                "Small",
                                self.fields[5].clone(),
                                "ツールバーなど、密度の高い場所に。",
                                cx,
                            ))
                            .child(field(
                                "Large",
                                self.fields[6].clone(),
                                "会話の入力など、主役になる場所に。",
                                cx,
                            )),
                    ),
                ),
            )
            .child(
                section(
                    "Textarea",
                    "会話と同じ複数行入力。Enter で改行、⌘ Enter で送信します。",
                    cx,
                )
                .child(
                    card(cx).p_5().gap_3().child(self.composer.clone()).child(
                        row()
                            .justify_between()
                            .child(label(format!("送信済み {} 件", self.submissions.len()), cx))
                            .child(
                                Button::new("composer-preview-send", "送信")
                                    .variant(ButtonVariant::Primary)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.composer
                                            .update(cx, |input, cx| input.submit(window, cx))
                                    })),
                            ),
                    ),
                ),
            )
            .child(alert(
                "日本語入力の確認",
                "変換、候補選択、確定、絵文字・結合文字の削除、コピーと貼付けを試せます。",
                Tone::Neutral,
                cx,
            ))
            .into_any_element()
    }

    pub(super) fn selection(&self, cx: &mut Context<Self>) -> AnyElement {
        stack()
            .child(
                section(
                    "Checkbox & switch",
                    "Checkbox は項目の選択に、Switch は即時に反映する設定に。",
                    cx,
                )
                .child(
                    div()
                        .flex()
                        .gap_5()
                        .child(
                            example("Checkbox", cx)
                                .flex_1()
                                .child(
                                    Toggle::checkbox(
                                        "check-interactive",
                                        "完了したセッションを含める",
                                        self.checked,
                                    )
                                    .on_change(cx.listener(
                                        |this, value: &bool, _, cx| {
                                            this.checked = *value;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    Toggle::checkbox("check-disabled", "変更できない項目", true)
                                        .disabled(true),
                                ),
                        )
                        .child(
                            example("Switch", cx)
                                .flex_1()
                                .child(
                                    Toggle::switch(
                                        "switch-interactive",
                                        "変更を自動保存する",
                                        self.switch_on,
                                    )
                                    .on_change(cx.listener(
                                        |this, value: &bool, _, cx| {
                                            this.switch_on = *value;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    Toggle::switch("switch-disabled-off", "通知をまとめる", false)
                                        .disabled(true),
                                )
                                .child(
                                    Toggle::switch("switch-disabled-on", "接続を維持する", true)
                                        .disabled(true),
                                ),
                        ),
                ),
            )
            .child(
                section(
                    "Radio",
                    "選択肢からひとつを選ぶ。矢印キーでも選択とフォーカスを移動できます。",
                    cx,
                )
                .child(
                    example("表示密度", cx).child(
                        gpui_kit::component::radio::RadioGroup::horizontal("density")
                            .selected_index(Some(self.radio))
                            .children(["コンパクト", "標準", "ゆったり"])
                            .on_change(cx.listener(|this, index: &usize, _, cx| {
                                this.radio = *index;
                                cx.notify();
                            })),
                    ),
                ),
            )
            .child(
                section(
                    "Select",
                    "上下キーで移動、Enter / Space で確定、Esc または外側のクリックで閉じる。",
                    cx,
                )
                .child(
                    card(cx).p_5().gap_3().child(
                        row()
                            .items_start()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(label("セッションを表示", cx))
                                    .child(self.select.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(label("Disabled", cx))
                                    .child(self.disabled_select.clone()),
                            ),
                    ),
                )
                .child(label("選択した値はこのアプリ内だけで保持されます。", cx)),
            )
            .into_any_element()
    }

    pub(super) fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme(cx);
        let tabs = ["概要", "アクティビティ", "設定"];
        stack()
            .child(
                section(
                    "Tabs",
                    "選択中の項目を面の違いで示し、補助操作を静かに置く。",
                    cx,
                )
                .child(
                    card(cx)
                        .p_5()
                        .gap_5()
                        .child(
                            TabBar::new("gallery-tabs")
                                .segmented()
                                .selected_index(self.selected_tab)
                                .children(tabs.map(|title| KitTab::new().label(title)))
                                .on_click(cx.listener(|this, index: &usize, _, cx| {
                                    this.selected_tab = *index;
                                    cx.notify();
                                })),
                        )
                        .child(divider(cx))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_4()
                                .p_3()
                                .child(Icon::Layout.view(p.accent_text))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(div().font_weight(FontWeight::MEDIUM).child(
                                            format!("{} を表示しています", tabs[self.selected_tab]),
                                        ))
                                        .child(label(
                                            "選択状態とコンテンツが一緒に切り替わります。",
                                            cx,
                                        )),
                                ),
                        ),
                ),
            )
            .child(
                section(
                    "Sidebar item",
                    "静かなナビゲーション。選択した場所だけを一段明るくする。",
                    cx,
                )
                .child(
                    card(cx).p_5().child(
                        div()
                            .flex()
                            .gap_6()
                            .child(
                                Sidebar::new("sidebar-example")
                                    .w(px(220.))
                                    .h(px(160.))
                                    .collapsible(false)
                                    .child(
                                        SidebarMenu::new().children(
                                            [
                                                (Icon::Folder, "Workspace"),
                                                (Icon::Layers, "Sessions"),
                                                (Icon::Settings, "Settings"),
                                            ]
                                            .into_iter()
                                            .enumerate()
                                            .map(
                                                |(i, (icon, title))| {
                                                    SidebarMenuItem::new(title)
                                                        .icon(icon.kit())
                                                        .active(self.selected_nav == i)
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.selected_nav = i;
                                                                cx.notify();
                                                            },
                                                        ))
                                                },
                                            ),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .gap_2()
                                    .child(div().font_weight(FontWeight::MEDIUM).child(
                                        ["Workspace", "Sessions", "Settings"][self.selected_nav],
                                    ))
                                    .child(label("Tab と Enter / Space でも選択できます。", cx)),
                            ),
                    ),
                ),
            )
            .child(
                section(
                    "Breadcrumbs & shortcuts",
                    "階層を短く表現し、よく使う操作にはショートカットを添える。",
                    cx,
                )
                .child(
                    example("Workspace / Solo / Design system", cx).child(
                        row()
                            .gap_1()
                            .child(
                                Button::new("crumb-workspace", "Workspace")
                                    .variant(ButtonVariant::Ghost)
                                    .with_icon(Icon::Folder)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message("Workspace を選択しました", Tone::Neutral, cx)
                                    })),
                            )
                            .child(Icon::ChevronRight.view(p.disabled).size(px(12.)))
                            .child(
                                Button::new("crumb-solo", "Solo")
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message("Solo を選択しました", Tone::Neutral, cx)
                                    })),
                            )
                            .child(Icon::ChevronRight.view(p.disabled).size(px(12.)))
                            .child(div().px_2().child("Design system"))
                            .child(div().flex_1())
                            .child(keycap("⌘ K", cx)),
                    ),
                ),
            )
            .into_any_element()
    }

    pub(super) fn feedback(&self, cx: &mut Context<Self>) -> AnyElement {
        stack()
            .child(
                section(
                    "Status & identity",
                    "色に加えてラベルでも意味を伝える。小さな要素を必要以上に強調しない。",
                    cx,
                )
                .child(
                    example("Badges / Avatars", cx)
                        .child(
                            row().children(
                                [
                                    ("待機中", Tone::Neutral),
                                    ("進行中", Tone::Accent),
                                    ("完了", Tone::Success),
                                    ("確認待ち", Tone::Warning),
                                    ("失敗", Tone::Danger),
                                ]
                                .into_iter()
                                .map(|(title, tone)| badge(title, tone, cx)),
                            ),
                        )
                        .child(
                            row()
                                .child(avatar("S", Tone::Accent, cx))
                                .child(icon_avatar(Icon::OpenAi, Tone::Accent, cx))
                                .child(avatar("AI", Tone::Neutral, cx))
                                .child(avatar("D", Tone::Success, cx)),
                        ),
                ),
            )
            .child(
                section(
                    "Progress & skeleton",
                    "完了率が分かる場合は進捗を、準備中は内容の輪郭を示す。",
                    cx,
                )
                .child(
                    div()
                        .flex()
                        .gap_5()
                        .child(
                            example("Determinate progress", cx)
                                .flex_1()
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .child(label("変更を準備しています", cx))
                                        .child(label(format!("{:.0}%", self.progress * 100.), cx)),
                                )
                                .child(progress(self.progress, Tone::Accent, cx))
                                .child(
                                    row()
                                        .child(
                                            Button::new("advance-progress", "進める")
                                                .control_size(ControlSize::Small)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.progress = (this.progress + 0.12).min(1.);
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("reset-progress", "リセット")
                                                .variant(ButtonVariant::Ghost)
                                                .control_size(ControlSize::Small)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.progress = 0.;
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            example("Content placeholder", cx).flex_1().child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .py_2()
                                    .child(
                                        div().size(px(30.)).rounded_full().bg(rgb(theme(cx).hover)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_3()
                                            .child(skeleton(170., cx))
                                            .child(skeleton(220., cx))
                                            .child(skeleton(120., cx)),
                                    ),
                            ),
                        ),
                ),
            )
            .child(
                section(
                    "Inline messages",
                    "状態と、次にできることを短く具体的に伝える。",
                    cx,
                )
                .child(alert(
                    "接続を確認してください",
                    "通信が切断されました。入力中の内容はこの画面に保持されています。",
                    Tone::Warning,
                    cx,
                ))
                .child(alert(
                    "変更を保存しました",
                    "ワークスペースの設定が更新されました。",
                    Tone::Success,
                    cx,
                ))
                .child(alert(
                    "名前を入力してください",
                    "プロジェクト名は3文字以上で入力できます。",
                    Tone::Danger,
                    cx,
                )),
            )
            .child(
                section("Empty state", "何もない理由と、最初の一歩を示す。", cx).child(
                    card(cx)
                        .child(empty_state(
                            Icon::Folder,
                            "セッションはまだありません",
                            "新しいセッションを作成して、作業を始めましょう。",
                            cx,
                        ))
                        .child(
                            div().flex().justify_center().pb_6().child(
                                Button::new("empty-create", "新しいセッション")
                                    .variant(ButtonVariant::Primary)
                                    .with_icon(Icon::Plus)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message(
                                            "新規作成の操作を確認しました",
                                            Tone::Success,
                                            cx,
                                        )
                                    })),
                            ),
                        ),
                ),
            )
            .into_any_element()
    }

    pub(super) fn overlays(&self, cx: &mut Context<Self>) -> AnyElement {
        stack()
            .child(section("Dialog", "背景の操作を止めて確認する。Tab はダイアログ内を移動し、閉じると元の操作へ戻ります。", cx)
                .child(card(cx).child(empty_state(Icon::Window, "大切な操作を、確かめてから。", "確認・取り消し・フォーカス復帰までをひとつのコンポーネントに。", cx))
                    .child(div().flex().justify_center().gap_3().pb_6()
                        .child(Button::new("open-dialog", "ダイアログを開く").variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, window, cx| this.dialog.update(cx, |dialog, cx| {
                                dialog.title = "変更を保存しますか？".into(); dialog.description = "ワークスペースの表示設定を保存します。後から設定画面で変更できます。".into(); dialog.confirm_label = "保存する".into(); dialog.destructive = false; dialog.show(window, cx);
                            }))))
                        .child(Button::new("open-danger-dialog", "削除の確認").variant(ButtonVariant::Secondary).with_icon(Icon::Trash)
                            .on_click(cx.listener(|this, _, window, cx| this.dialog.update(cx, |dialog, cx| {
                                dialog.title = "セッションを削除しますか？".into(); dialog.description = "これはデザイン確認用のダイアログです。実際のセッションやファイルは変更されません。".into(); dialog.confirm_label = "削除する".into(); dialog.destructive = true; dialog.show(window, cx);
                            })))))))
            .child(section("Toast", "操作の結果を短く通知する。フォーカスを移さず、4秒後に閉じます。", cx)
                .child(example("Try a notification", cx).child(row().children([
                    ("保存完了", "変更を保存しました", Tone::Success), ("お知らせ", "表示設定を更新しました", Tone::Neutral), ("注意", "接続が不安定です", Tone::Warning), ("エラー", "保存できませんでした", Tone::Danger),
                ].into_iter().enumerate().map(|(i, (title, message, tone))| {
                    Button::new(("toast-example", i), title).on_click(cx.listener(move |this, _, _, cx| this.message(message, tone, cx)))
                })))))
            .child(section("Tooltip", "アイコンの意味を補足する。主操作に必要な情報は隠さない。", cx)
                .child(example("Hover to inspect", cx).child(row()
                    .child(Button::icon("tooltip-copy", Icon::Copy, "リンクをコピー").on_click(cx.listener(|this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string("solo://design/overlays".into())); this.message("サンプルリンクをコピーしました", Tone::Success, cx);
                    })))
                    .child(Button::icon("tooltip-settings", Icon::Settings, "表示設定").on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Foundations, cx))))
                    .child(label("アイコンにマウスを合わせると、操作名を表示します。", cx)))))
            .into_any_element()
    }
}
