mod pages;
mod smoke;

use gpui::{prelude::*, *};
use solo::design::{self as ds, *};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Overview,
    Foundations,
    Buttons,
    Inputs,
    Selection,
    Navigation,
    Feedback,
    Overlays,
}
impl Page {
    const ALL: [Self; 8] = [
        Self::Overview,
        Self::Foundations,
        Self::Buttons,
        Self::Inputs,
        Self::Selection,
        Self::Navigation,
        Self::Feedback,
        Self::Overlays,
    ];
    fn slug(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Foundations => "foundations",
            Self::Buttons => "buttons",
            Self::Inputs => "inputs",
            Self::Selection => "selection",
            Self::Navigation => "navigation",
            Self::Feedback => "feedback",
            Self::Overlays => "overlays",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Foundations => "Foundations",
            Self::Buttons => "Buttons",
            Self::Inputs => "Inputs",
            Self::Selection => "Selection",
            Self::Navigation => "Navigation",
            Self::Feedback => "Feedback",
            Self::Overlays => "Overlays",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Overview => "集中できる画面は、小さな一貫性から。",
            Self::Foundations => "色、文字、余白。すべてのコンポーネントに共通する基準。",
            Self::Buttons => "操作の優先順位を、控えめで明確なコントラストで伝える。",
            Self::Inputs => "日本語入力と、入力前・入力中・検証後の状態を整える。",
            Self::Selection => "選択する、切り替える。状態が迷わず伝わるコントロール。",
            Self::Navigation => "今いる場所と次の行き先を、さりげなく示す。",
            Self::Feedback => "進行状況、結果、次に必要な操作を伝える。",
            Self::Overlays => "必要なときだけ現れ、作業へ自然に戻れる小さな画面。",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Self::Overview => Icon::Grid,
            Self::Foundations => Icon::Layers,
            Self::Buttons => Icon::Cursor,
            Self::Inputs => Icon::Text,
            Self::Selection => Icon::Sliders,
            Self::Navigation => Icon::Layout,
            Self::Feedback => Icon::Bell,
            Self::Overlays => Icon::Window,
        }
    }
}

actions!(design_gallery, [QuitGallery, CloseGallery, ToggleTheme]);

pub fn run() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "Solo Design\n  --light\n  --page overview|foundations|buttons|inputs|selection|navigation|feedback|overlays\n  --smoke\n  --compact"
        );
        return;
    }
    let page = args
        .windows(2)
        .find(|pair| pair[0] == "--page")
        .map(|pair| {
            Page::ALL
                .into_iter()
                .find(|p| p.slug() == pair[1])
                .unwrap_or_else(|| {
                    eprintln!("不明な page: {}", pair[1]);
                    std::process::exit(2);
                })
        })
        .unwrap_or(Page::Overview);
    let light = args.iter().any(|arg| arg == "--light");
    let smoke = args.iter().any(|arg| arg == "--smoke");
    let compact = args.iter().any(|arg| arg == "--compact");
    Application::new().with_assets(DesignAssets).run(move |cx| {
        ds::init(cx);
        if light {
            ds::set_theme(ColorScheme::Light, cx);
        }
        cx.bind_keys([
            KeyBinding::new("cmd-q", QuitGallery, None),
            KeyBinding::new("cmd-w", CloseGallery, None),
            KeyBinding::new("cmd-shift-l", ToggleTheme, None),
        ]);
        cx.on_action(|_: &QuitGallery, cx| cx.quit());
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let window_size = if compact {
            size(px(980.), px(720.))
        } else {
            size(px(1280.), px(900.))
        };
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    window_size,
                    cx,
                ))),
                window_min_size: Some(size(px(980.), px(700.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Solo Design".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(16.))),
                }),
                app_id: Some("dev.solo.design".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Gallery::new(page, smoke, window, cx)),
        )
        .expect("Solo Design の window を開けませんでした");
        cx.activate(true);
    });
}

struct Gallery {
    page: Page,
    focus: FocusHandle,
    search: Entity<TextInput>,
    query: String,
    fields: Vec<Entity<TextInput>>,
    select: Entity<Select>,
    disabled_select: Entity<Select>,
    dialog: Entity<Dialog>,
    toast: Entity<ToastHost>,
    scrolls: [ScrollHandle; 8],
    checked: bool,
    switch_on: bool,
    mixed: bool,
    radio: usize,
    radio_focus: [FocusHandle; 3],
    selected_tab: usize,
    selected_nav: usize,
    progress: f32,
    clicks: usize,
    loading: bool,
    loading_task: Option<Task<()>>,
    probe_focus: FocusHandle,
    disabled_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    rendered: usize,
}
impl Gallery {
    fn new(page: Page, smoke: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(cx)
                .control_size(ControlSize::Small)
                .leading_icon(Icon::Search)
                .placeholder("コンポーネントを探す")
        });
        let fields = vec![
            cx.new(|cx| {
                TextInput::new(cx)
                    .placeholder("プロジェクト名")
                    .default_value("Solo workspace")
            }),
            cx.new(|cx| {
                TextInput::new(cx)
                    .placeholder("セッションを検索…")
                    .leading_icon(Icon::Search)
            }),
            cx.new(|cx| {
                TextInput::new(cx)
                    .placeholder("3文字以上の名前")
                    .default_value("ab")
                    .invalid(true)
            }),
            cx.new(|cx| {
                TextInput::new(cx)
                    .default_value("変更できません")
                    .disabled(true)
            }),
            cx.new(|cx| {
                TextInput::new(cx)
                    .default_value("solo / design-system")
                    .read_only(true)
            }),
            cx.new(|cx| {
                TextInput::new(cx)
                    .placeholder("Small · 28 px")
                    .control_size(ControlSize::Small)
            }),
            cx.new(|cx| {
                TextInput::new(cx)
                    .placeholder("Large · 40 px")
                    .control_size(ControlSize::Large)
            }),
        ];
        let select = cx.new(|cx| {
            Select::new(
                ["すべてのセッション", "進行中", "完了", "アーカイブ"],
                0,
                cx,
            )
        });
        let disabled_select = cx.new(|cx| Select::new(["選択できません"], 0, cx).disabled(true));
        let dialog = cx.new(Dialog::new);
        let toast = cx.new(ToastHost::new);
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, event: &InputChanged, cx| {
                this.query = event.text.to_lowercase();
                cx.notify();
            }),
            cx.subscribe(&fields[2], |_, input, event: &InputChanged, cx| {
                if !event.composing {
                    input.update(cx, |input, cx| {
                        input.invalid = event.text.chars().count() < 3;
                        cx.notify();
                    });
                }
                cx.notify();
            }),
            cx.subscribe(&fields[0], |_, _, _: &InputChanged, cx| cx.notify()),
            cx.subscribe(&fields[0], |this, _, _: &Submitted, cx| {
                this.message("入力内容を保存しました", Tone::Success, cx)
            }),
            cx.subscribe(&select, |this, _, event: &SelectionChanged, cx| {
                this.message(format!("{} を選択しました", event.label), Tone::Neutral, cx)
            }),
            cx.subscribe(&dialog, |this, _, result: &DialogEvent, cx| {
                if *result == DialogEvent::Confirmed {
                    this.message("変更を保存しました", Tone::Success, cx);
                }
            }),
        ];
        let focus = cx.focus_handle();
        window.focus(&focus);
        let this = Self {
            page,
            focus,
            search,
            query: String::new(),
            fields,
            select,
            disabled_select,
            dialog,
            toast,
            scrolls: std::array::from_fn(|_| ScrollHandle::new()),
            checked: true,
            switch_on: true,
            mixed: true,
            radio: 0,
            radio_focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            selected_tab: 0,
            selected_nav: 0,
            progress: 0.64,
            clicks: 0,
            loading: false,
            loading_task: None,
            probe_focus: cx.focus_handle().tab_stop(true),
            disabled_focus: cx.focus_handle().tab_stop(false),
            _subscriptions: subscriptions,
            rendered: 0,
        };
        if smoke {
            smoke::start(window, cx);
        }
        this
    }
    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.select.update(cx, |select, cx| select.close(cx));
        cx.notify();
    }
    fn message(&mut self, message: impl Into<SharedString>, tone: Tone, cx: &mut Context<Self>) {
        self.toast
            .update(cx, |toast, cx| toast.push(message, tone, cx));
    }
    fn record_click(&mut self, cx: &mut Context<Self>) {
        self.clicks += 1;
        self.message(
            format!("アクションを実行しました · {} 回", self.clicks),
            Tone::Success,
            cx,
        );
        cx.notify();
    }
    fn start_loading(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.loading_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                this.message("保存が完了しました", Tone::Success, cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme(cx);
        let matches: Vec<_> = Page::ALL
            .into_iter()
            .filter(|page| {
                format!("{} {}", page.title(), page.description())
                    .to_lowercase()
                    .contains(&self.query)
            })
            .collect();
        div()
            .w(px(220.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(p.sidebar))
            .border_r_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .h(px(46.))
                    .flex_shrink_0()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                div()
                    .px_5()
                    .pt_3()
                    .pb_5()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(7.))
                            .bg(rgb(p.text))
                            .text_color(rgb(p.sidebar))
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .child("s"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Solo Design"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(p.muted))
                                    .child("COMPONENT LIBRARY"),
                            ),
                    ),
            )
            .child(div().px_3().mb_5().child(self.search.clone()))
            .child(
                div()
                    .px_5()
                    .mb_2()
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(p.muted))
                    .child("ライブラリ"),
            )
            .child(
                div()
                    .id("gallery-navigation")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .children(matches.iter().map(|&page| {
                        nav_item(
                            ("page", page as usize),
                            page.icon(),
                            page.title(),
                            self.page == page,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
                    }))
                    .when(matches.is_empty(), |v| {
                        v.child(
                            div()
                                .px_5()
                                .py_3()
                                .text_color(rgb(p.muted))
                                .text_size(px(12.))
                                .child("該当する項目がありません"),
                        )
                    }),
            )
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(11.))
                            .text_color(rgb(p.muted))
                            .child("Appearance")
                            .child(keycap("⌘ ⇧ L", cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .p_1()
                            .rounded(px(radius::CONTROL))
                            .bg(rgb(p.surface))
                            .children(
                                [ColorScheme::Light, ColorScheme::Dark]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, scheme)| {
                                        tab(
                                            ("scheme", i),
                                            scheme.label(),
                                            ds::scheme(cx) == scheme,
                                            cx,
                                        )
                                        .with_icon(if scheme == ColorScheme::Dark {
                                            Icon::Moon
                                        } else {
                                            Icon::Sun
                                        })
                                        .flex_1()
                                        .on_click(move |_, _, cx| ds::set_theme(scheme, cx))
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(10.))
                            .text_color(rgb(p.muted))
                            .child(div().size(px(5.)).rounded_full().bg(rgb(p.success)))
                            .child("v0.1  ·  Shared with Solo"),
                    ),
            )
            .into_any_element()
    }
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered += 1;
        let p = theme(cx);
        let content = match self.page {
            Page::Overview => self.overview(cx),
            Page::Foundations => self.foundations(cx),
            Page::Buttons => self.buttons(cx),
            Page::Inputs => self.inputs(cx),
            Page::Selection => self.selection(cx),
            Page::Navigation => self.navigation(cx),
            Page::Feedback => self.feedback(cx),
            Page::Overlays => self.overlays(cx),
        };
        ds::root(cx)
            .relative()
            .track_focus(&self.focus)
            .flex()
            .on_action(|_: &CloseGallery, window, _| window.remove_window())
            .on_action(|_: &ToggleTheme, _, cx| ds::set_theme(ds::scheme(cx).opposite(), cx))
            .child(self.sidebar(cx))
            .child(
                div()
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(46.))
                            .flex_shrink_0()
                            .px_6()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(rgb(p.border))
                            .window_control_area(WindowControlArea::Drag)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(12.))
                                    .text_color(rgb(p.muted))
                                    .child("Design system")
                                    .child(Icon::ChevronRight.view(p.disabled).size(px(12.)))
                                    .child(div().text_color(rgb(p.text)).child(self.page.title())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(badge("Preview", Tone::Neutral, cx))
                                    .child(
                                        Button::icon(
                                            "theme-toggle",
                                            if ds::scheme(cx) == ColorScheme::Dark {
                                                Icon::Sun
                                            } else {
                                                Icon::Moon
                                            },
                                            "テーマを切り替える · ⌘ ⇧ L",
                                        )
                                        .control_size(ControlSize::Small)
                                        .on_click(
                                            |_, _, cx| ds::set_theme(ds::scheme(cx).opposite(), cx),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id(("page-scroll", self.page as usize))
                            .track_scroll(&self.scrolls[self.page as usize])
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(1120.))
                                    .mx_auto()
                                    .px(px(40.))
                                    .pt(px(36.))
                                    .pb(px(48.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(28.))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(rgb(p.accent_text))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(if self.page == Page::Overview {
                                                        "SOLO / DESIGN FOUNDATIONS"
                                                    } else {
                                                        "COMPONENTS / LIBRARY"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap_4()
                                                    .child(
                                                        div()
                                                            .text_size(px(30.))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child(
                                                                if self.page == Page::Overview {
                                                                    "小さな一貫性、大きな心地よさ。"
                                                                } else {
                                                                    self.page.title()
                                                                },
                                                            ),
                                                    )
                                                    .when(self.page != Page::Overview, |v| {
                                                        v.child(badge("Ready", Tone::Success, cx))
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .line_height(px(22.))
                                                    .text_color(rgb(p.secondary))
                                                    .child(self.page.description()),
                                            ),
                                    )
                                    .child(content),
                            ),
                    )
                    .child(
                        div()
                            .h(px(30.))
                            .px_6()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_t_1()
                            .border_color(rgb(p.border))
                            .text_size(px(10.))
                            .text_color(rgb(p.muted))
                            .child("Tab で移動   ·   Enter / Space で操作   ·   Esc で閉じる")
                            .child(format!("{} theme  ·  GPUI", ds::scheme(cx).label())),
                    ),
            )
            .child(self.toast.clone())
            .child(self.dialog.clone())
    }
}

fn stack() -> Div {
    div().w_full().flex().flex_col().gap(px(24.))
}
fn row() -> Div {
    div().flex().flex_wrap().items_center().gap(px(12.))
}
fn section(title: &'static str, description: &'static str, cx: &App) -> Div {
    let p = theme(cx);
    div().flex().flex_col().gap(px(16.)).child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .child(description),
            ),
    )
}
fn label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(11.))
        .text_color(rgb(theme(cx).muted))
        .child(text.into())
}
fn example(title: &'static str, cx: &App) -> Div {
    card(cx).p_5().gap_4().child(
        div()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme(cx).secondary))
            .child(title),
    )
}
fn field(label_text: &'static str, input: Entity<TextInput>, hint: &'static str, cx: &App) -> Div {
    div()
        .flex_1()
        .min_w(px(240.))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .child(label_text),
        )
        .child(input)
        .child(label(hint, cx))
}
