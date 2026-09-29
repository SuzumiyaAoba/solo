mod pages;
mod smoke;

use gpui_kit::component::{
    Sizable,
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    tab::{Tab as KitTab, TabBar},
};
use gpui_kit::{prelude::*, *};
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

/// 各ページの表示情報。サイドバーと本文ヘッダで共通して使う。
struct PageInfo {
    slug: &'static str,
    title: &'static str,
    description: &'static str,
    icon: Icon,
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

    fn info(self) -> PageInfo {
        match self {
            Self::Overview => PageInfo {
                slug: "overview",
                title: "Overview",
                description: "集中できる画面は、小さな一貫性から。",
                icon: Icon::Grid,
            },
            Self::Foundations => PageInfo {
                slug: "foundations",
                title: "Foundations",
                description: "色、文字、余白。すべてのコンポーネントに共通する基準。",
                icon: Icon::Layers,
            },
            Self::Buttons => PageInfo {
                slug: "buttons",
                title: "Buttons",
                description: "操作の優先順位を、控えめで明確なコントラストで伝える。",
                icon: Icon::Cursor,
            },
            Self::Inputs => PageInfo {
                slug: "inputs",
                title: "Inputs",
                description: "日本語入力と、入力前・入力中・検証後の状態を整える。",
                icon: Icon::Text,
            },
            Self::Selection => PageInfo {
                slug: "selection",
                title: "Selection",
                description: "選択する、切り替える。状態が迷わず伝わるコントロール。",
                icon: Icon::Sliders,
            },
            Self::Navigation => PageInfo {
                slug: "navigation",
                title: "Navigation",
                description: "今いる場所と次の行き先を、さりげなく示す。",
                icon: Icon::Layout,
            },
            Self::Feedback => PageInfo {
                slug: "feedback",
                title: "Feedback",
                description: "進行状況、結果、次に必要な操作を伝える。",
                icon: Icon::Bell,
            },
            Self::Overlays => PageInfo {
                slug: "overlays",
                title: "Overlays",
                description: "必要なときだけ現れ、作業へ自然に戻れる小さな画面。",
                icon: Icon::Window,
            },
        }
    }

    fn slug(self) -> &'static str {
        self.info().slug
    }
    fn title(self) -> &'static str {
        self.info().title
    }
    fn description(self) -> &'static str {
        self.info().description
    }
    fn icon(self) -> Icon {
        self.info().icon
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
    gpui_kit::application()
        .with_assets(DesignAssets)
        .run(move |cx| {
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
            cx.on_window_closed(|cx, _| {
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
                |window, cx| {
                    let view = cx.new(|cx| Gallery::new(page, smoke, window, cx));
                    cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
                },
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
    composer: Entity<TextInput>,
    submissions: Vec<String>,
    select: Entity<Select>,
    disabled_select: Entity<Select>,
    dialog: Entity<Dialog>,
    toast: Entity<ToastHost>,
    scrolls: [ScrollHandle; 8],
    checked: bool,
    switch_on: bool,
    radio: usize,
    selected_tab: usize,
    selected_nav: usize,
    progress: f32,
    clicks: usize,
    loading: bool,
    loading_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
    rendered: usize,
}
impl Gallery {
    fn new(page: Page, smoke: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(window, cx)
                .control_size(ControlSize::Small)
                .leading_icon(Icon::Search)
                .placeholder("コンポーネントを探す")
        });
        let fields = vec![
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .placeholder("プロジェクト名")
                    .default_value("Solo workspace")
            }),
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .placeholder("セッションを検索…")
                    .leading_icon(Icon::Search)
            }),
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .placeholder("3文字以上の名前")
                    .default_value("ab")
                    .invalid(true)
            }),
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .default_value("変更できません")
                    .disabled(true)
            }),
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .default_value("solo / design-system")
                    .read_only(true)
            }),
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .placeholder("Small · 28 px")
                    .control_size(ControlSize::Small)
            }),
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .placeholder("Large · 40 px")
                    .control_size(ControlSize::Large)
            }),
        ];
        let composer = cx.new(|cx| {
            TextInput::multiline(window, cx)
                .placeholder("作業を依頼する…")
                .clear_on_submit(true)
        });
        let select = cx.new(|cx| {
            Select::new(
                ["すべてのセッション", "進行中", "完了", "アーカイブ"],
                0,
                window,
                cx,
            )
        });
        let disabled_select =
            cx.new(|cx| Select::new(["選択できません"], 0, window, cx).disabled(true));
        let dialog = cx.new(Dialog::new);
        let toast = cx.new(ToastHost::new);
        let subscriptions = vec![
            cx.subscribe(&composer, |this, _, submitted: &Submitted, cx| {
                this.submissions.push(submitted.0.clone());
                this.message("メッセージを送信しました", Tone::Success, cx);
            }),
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
        window.focus(&focus, cx);
        let this = Self {
            page,
            focus,
            search,
            query: String::new(),
            fields,
            composer,
            submissions: Vec::new(),
            select,
            disabled_select,
            dialog,
            toast,
            scrolls: std::array::from_fn(|_| ScrollHandle::new()),
            checked: true,
            switch_on: true,
            radio: 0,
            selected_tab: 0,
            selected_nav: 0,
            progress: 0.64,
            clicks: 0,
            loading: false,
            loading_task: None,
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
        let matches: Vec<_> = Page::ALL
            .into_iter()
            .filter(|page| {
                format!("{} {}", page.title(), page.description())
                    .to_lowercase()
                    .contains(&self.query)
            })
            .collect();
        let menu = SidebarMenu::new().children(matches.iter().map(|&page| {
            SidebarMenuItem::new(page.title())
                .icon(page.icon().kit())
                .active(self.page == page)
                .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
        }));
        Sidebar::new("gallery-sidebar")
            .w(px(220.))
            .h_full()
            .collapsible(false)
            .header(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .pb_3()
                    .child(
                        div()
                            .h(px(46.))
                            .window_control_area(WindowControlArea::Drag),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_2()
                            .child(avatar("S", Tone::Accent, cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Solo Design"),
                                    )
                                    .child(label("GPUI Kit 0.6.6", cx)),
                            ),
                    )
                    .child(self.search.clone()),
            )
            .child(
                SidebarGroup::new(if matches.is_empty() {
                    "該当する項目がありません"
                } else {
                    "ライブラリ"
                })
                .child(menu),
            )
            .footer(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_2()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(label("テーマ", cx))
                            .child(keycap("⌘ ⇧ L", cx)),
                    )
                    .child(
                        TabBar::new("gallery-appearance")
                            .segmented()
                            .small()
                            .selected_index(usize::from(ds::scheme(cx) == ColorScheme::Dark))
                            .child(KitTab::new().label("Light"))
                            .child(KitTab::new().label("Dark"))
                            .on_click(|index, _, cx| {
                                ds::set_theme(
                                    if *index == 0 {
                                        ColorScheme::Light
                                    } else {
                                        ColorScheme::Dark
                                    },
                                    cx,
                                )
                            }),
                    )
                    .child(label("メインアプリと同じコンポーネント", cx)),
            )
            .into_any_element()
    }
}
impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                    .text_size(px(typography::LABEL))
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
                                    .px(px(space::SECTION))
                                    .pt(px(space::XXL))
                                    .pb(px(space::SECTION))
                                    .flex()
                                    .flex_col()
                                    .gap(px(space::XXL))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(typography::CAPTION))
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
                                                            .text_size(px(typography::TITLE))
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
                                                    .text_size(px(typography::BODY))
                                                    .line_height(px(20.))
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
                            .text_size(px(typography::CAPTION))
                            .text_color(rgb(p.muted))
                            .child("Tab で移動   ·   Enter / Space で操作   ·   Esc で閉じる")
                            .child(format!("{} theme  ·  GPUI Kit", ds::scheme(cx).label())),
                    ),
            )
            .child(self.toast.clone())
            .children(gpui_kit::component::Root::render_dialog_layer(window, cx))
            .children(gpui_kit::component::Root::render_notification_layer(
                window, cx,
            ))
            .child(self.dialog.clone())
    }
}

fn stack() -> Div {
    div().w_full().flex().flex_col().gap(px(space::XL))
}
fn row() -> Div {
    div().flex().flex_wrap().items_center().gap(px(space::MD))
}
fn section(title: &'static str, description: &'static str, cx: &App) -> Div {
    let p = theme(cx);
    div().flex().flex_col().gap(px(space::LG)).child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(typography::LEAD))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(typography::LABEL))
                    .text_color(rgb(p.muted))
                    .child(description),
            ),
    )
}
fn label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(typography::CAPTION))
        .text_color(rgb(theme(cx).muted))
        .child(text.into())
}
fn example(title: &'static str, cx: &App) -> Div {
    card(cx).p_6().gap_4().child(
        div()
            .text_size(px(typography::LABEL))
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
                .text_size(px(typography::LABEL))
                .font_weight(FontWeight::MEDIUM)
                .child(label_text),
        )
        .child(input)
        .child(label(hint, cx))
}
