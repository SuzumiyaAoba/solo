use gpui::{prelude::*, *};
use solo::design::{
    self, Button, ButtonVariant, ControlSize, DesignAssets, Submitted, TextInput as Composer,
};
use solo::{
    mock::{self, Config, Controller, Delivery, FRAME_BATCH, FRAME_INTERVAL, Scenario},
    projection::{DiffKind, Session, Speaker, Status},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tempfile::TempPath;

const BG: u32 = design::DARK.canvas;
const PANEL: u32 = design::DARK.surface;
const BORDER: u32 = design::DARK.border;
const MUTED: u32 = design::DARK.muted;
const ACCENT: u32 = design::DARK.accent_text;

actions!(solo_app, [Quit, CloseWindow, NewSession]);

pub fn run() {
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    Application::new().with_assets(DesignAssets).run(move |cx| {
        design::init(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-w", CloseWindow, None),
            KeyBinding::new("cmd-n", NewSession, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1180.), px(780.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(820.), px(560.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Solo — Phase 0".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Workspace::new(smoke, window, cx)),
        )
        .expect("Solo の window を開けませんでした");
        cx.activate(true);
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Chat,
    Diff,
    Logs,
}

struct SessionView {
    model: Session,
    composer: Entity<Composer>,
    chat_list: ListState,
    log_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
    diff_index: usize,
    tab: Tab,
    follow_logs: bool,
    artifacts: Vec<Arc<TempPath>>,
    controller: Option<Controller>,
    task: Option<Task<()>>,
    _input_subscription: Subscription,
    batches: u64,
    max_batch_ms: f64,
}

struct Workspace {
    sessions: Vec<SessionView>,
    selected: usize,
    serial: u64,
    message: String,
    smoke: bool,
    rendered: usize,
}

impl Workspace {
    fn new(smoke: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            sessions: Vec::new(),
            selected: 0,
            serial: 0,
            message: String::new(),
            smoke,
            rendered: 0,
        };
        this.new_session(cx);
        window.focus(&this.sessions[0].composer.focus_handle(cx));
        this.start(
            0,
            Scenario::Demo,
            "Phase 0 の表示を確認してください。".into(),
            cx,
        );
        if smoke {
            // 実 window 上で全 pane と負荷中の入力 handler を通す。
            // OS の候補選択や input→present 計測は別の手動受入。
            cx.spawn_in(window, async move |this, cx| {
                for scenario in [Scenario::Demo, Scenario::Events100k, Scenario::Log100MiB, Scenario::Faults] {
                    if scenario != Scenario::Demo {
                        this.update_in(cx, |this, _, cx| this.scenario(scenario, cx)).expect("smoke window remains open");
                    }
                    let started = Instant::now();
                    let mut tick = 0;
                    loop {
                        cx.background_executor().timer(Duration::from_millis(100)).await;
                        let done = this.update_in(cx, |this, window, cx| {
                            let session = &mut this.sessions[0];
                            session.tab = [Tab::Chat, Tab::Logs, Tab::Diff][tick % 3];
                            session.composer.update(cx, |input, cx| {
                                input.buffer = Default::default();
                                input.replace_text_in_range(None, "前🙂", window, cx);
                                input.replace_and_mark_text_in_range(None, "にほんご", Some(4..4), window, cx);
                                assert_eq!(input.marked_text_range(window, cx), Some(3..7));
                                assert!(input.buffer.take_committed().is_none());
                                input.replace_text_in_range(None, "日本語", window, cx);
                                assert_eq!(input.buffer.content, "前🙂日本語");
                            });
                            let done = !session.model.status.is_active();
                            if done {
                                assert_eq!(session.model.status, if scenario == Scenario::Faults { Status::Disconnected } else { Status::Completed });
                                assert!(!session.model.diffs.is_empty());
                                println!("GPUI smoke: {} / events={} / renders={} / max batch={:.2}ms", scenario.label(), session.model.accepted, this.rendered, session.max_batch_ms);
                            }
                            cx.notify();
                            done
                        }).expect("smoke window remains open");
                        if done { break; }
                        assert!(started.elapsed() < Duration::from_secs(45), "smoke stream timed out");
                        tick += 1;
                    }
                }
                this.update_in(cx, |this, _, cx| {
                    assert!(this.rendered > 10);
                    assert_eq!(this.sessions[0].model.unknown, 2);
                    assert_eq!(this.sessions[0].model.rejected, 2);
                    println!("GPUI smoke OK: all panes, 100k events, 100MiB logs, input handler, disconnect");
                    cx.quit();
                }).expect("smoke window remains open");
            })
            .detach();
        }
        this
    }

    fn new_session(&mut self, cx: &mut Context<Self>) {
        if self.sessions.len() >= 8 {
            self.message = "最大8セッションです。不要なセッションを閉じてください。".into();
            cx.notify();
            return;
        }
        self.serial += 1;
        let id = format!("session-{}", self.serial);
        let composer = cx.new(|cx| {
            Composer::new(cx)
                .control_size(ControlSize::Large)
                .clear_on_submit(true)
        });
        let callback_id = id.clone();
        let subscription = cx.subscribe(&composer, move |this, _, submitted: &Submitted, cx| {
            if let Some(index) = this.sessions.iter().position(|s| s.model.id == callback_id) {
                this.start(index, Scenario::Demo, submitted.0.clone(), cx);
            }
        });
        self.sessions.push(SessionView {
            model: Session::new(id, format!("新しいセッション {}", self.serial)),
            composer,
            chat_list: ListState::new(0, ListAlignment::Bottom, px(300.)),
            log_scroll: UniformListScrollHandle::new(),
            diff_scroll: UniformListScrollHandle::new(),
            diff_index: 0,
            tab: Tab::Chat,
            follow_logs: true,
            artifacts: Vec::new(),
            controller: None,
            task: None,
            _input_subscription: subscription,
            batches: 0,
            max_batch_ms: 0.,
        });
        self.selected = self.sessions.len() - 1;
        cx.notify();
    }

    fn start(&mut self, index: usize, scenario: Scenario, prompt: String, cx: &mut Context<Self>) {
        let session = &mut self.sessions[index];
        if session.model.status.is_active() {
            return;
        }
        let mut config = Config::new(&session.model.id, scenario);
        config.title = format!(
            "{} / {}",
            session.model.id.trim_start_matches("session-"),
            scenario.label()
        );
        config.prompt = prompt.clone();
        config.start_sequence = session.model.last_sequence;
        config.workspace = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "solo".into());
        let (controller, receiver) = match mock::start(config) {
            Ok(stream) => stream,
            Err(error) => {
                session.composer.update(cx, |input, cx| {
                    input.buffer.replace(None, &prompt);
                    cx.notify();
                });
                self.message = format!("疑似ストリームを開始できませんでした: {error}");
                cx.notify();
                return;
            }
        };
        session.model.status = Status::Connecting;
        session.model.reason.clear();
        session.composer.update(cx, |input, cx| {
            input.can_submit = false;
            cx.notify();
        });
        session.controller = Some(controller);
        let id = session.model.id.clone();
        session.task = Some(cx.spawn(async move |this, cx| {
            loop {
                let first = match receiver.recv().await {
                    Ok(event) => event,
                    Err(_) => {
                        let _ = this.update(cx, |this, cx| this.consume(&id, Vec::new(), true, cx));
                        break;
                    }
                };
                // 空のときは recv で待つ。入力や描画を阻害せず、delta をフレーム単位でまとめる。
                cx.background_executor().timer(FRAME_INTERVAL).await;
                let mut batch = Vec::with_capacity(FRAME_BATCH);
                batch.push(first);
                while batch.len() < FRAME_BATCH {
                    match receiver.try_recv() {
                        Ok(event) => batch.push(event),
                        Err(_) => break,
                    }
                }
                if this
                    .update(cx, |this, cx| this.consume(&id, batch, false, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn consume(&mut self, id: &str, batch: Vec<Delivery>, closed: bool, cx: &mut Context<Self>) {
        let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.model.id == id)
        else {
            return;
        };
        let start = Instant::now();
        let old_len = session.model.chat.len();
        let old_discarded = session.model.chat_discarded;
        for delivery in batch {
            match delivery {
                Delivery::Event(event) => {
                    session.model.apply(event);
                }
                Delivery::LogOpened(path) => session.artifacts.push(path),
                Delivery::Error(error) => {
                    session.model.status = Status::Failed;
                    session.model.reason = error;
                }
            }
        }
        if closed {
            session.model.transport_closed();
        }
        let discarded = session.model.chat_discarded - old_discarded;
        if discarded > 0 {
            session.chat_list.splice(0..discarded.min(old_len), 0);
        }
        let old_len = old_len.saturating_sub(discarded);
        let from = old_len.saturating_sub(1);
        session
            .chat_list
            .splice(from..old_len, session.model.chat.len() - from);
        if session.follow_logs && !session.model.logs.is_empty() {
            session
                .log_scroll
                .scroll_to_item(session.model.logs.len() - 1, ScrollStrategy::Bottom);
        }
        let can_submit = !session.model.status.is_active();
        if session.composer.read(cx).can_submit != can_submit {
            session.composer.update(cx, |input, cx| {
                input.can_submit = can_submit;
                cx.notify();
            });
        }
        session.batches += 1;
        session.max_batch_ms = session
            .max_batch_ms
            .max(start.elapsed().as_secs_f64() * 1000.);
        cx.notify();
    }

    fn scenario(&mut self, scenario: Scenario, cx: &mut Context<Self>) {
        if self.sessions[self.selected].model.status.is_active() {
            return;
        }
        self.start(
            self.selected,
            scenario,
            format!("{}の表示と操作を検証します。", scenario.label()),
            cx,
        );
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(224.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .px_5()
                    .py_5()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ACCENT))
                            .child("solo"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .mt_1()
                            .child("ひとつの作業に、集中する。"),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .mb_4()
                    .child(
                        button("new", "＋ 新しいセッション")
                            .w_full()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_session(cx);
                                window
                                    .focus(&this.sessions[this.selected].composer.focus_handle(cx));
                            })),
                    ),
            )
            .child(
                div()
                    .px_5()
                    .mb_2()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(format!("セッション  {} / 8", self.sessions.len())),
            )
            .child(
                div()
                    .id("sessions")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .children(self.sessions.iter().enumerate().map(|(index, session)| {
                        div()
                            .id(("session", index))
                            .px_3()
                            .py_3()
                            .mb_1()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(rgb(if index == self.selected {
                                design::DARK.hover
                            } else {
                                PANEL
                            }))
                            .hover(|style| style.bg(rgb(design::DARK.hover)))
                            .child(
                                div()
                                    .text_sm()
                                    .truncate()
                                    .child(session.model.title.clone()),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .text_xs()
                                    .text_color(rgb(status_color(session.model.status)))
                                    .child(format!(
                                        "{}  ·  {} events",
                                        session.model.status.label(),
                                        session.model.accepted
                                    )),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.selected = index;
                                window.focus(&this.sessions[index].composer.focus_handle(cx));
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(
                        div()
                            .text_color(rgb(ACCENT))
                            .child("PHASE 0  /  ローカル試作"),
                    )
                    .child(
                        div()
                            .mt_2()
                            .child("疑似イベントで会話・差分・ログを確認できます。"),
                    ),
            )
    }

    fn conversation(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.sessions[self.selected];
        if session.model.chat.is_empty() {
            return empty(
                "作業を始めましょう",
                "メッセージを入力するか、上部から試すシナリオを選んでください。",
            )
            .into_any_element();
        }
        let weak = cx.weak_entity();
        let index = self.selected;
        list(session.chat_list.clone(), move |row, _, cx| {
            let Some(view) = weak.upgrade() else {
                return div().into_any_element();
            };
            let Some(block) = view
                .read(cx)
                .sessions
                .get(index)
                .and_then(|s| s.model.chat.get(row))
            else {
                return div().into_any_element();
            };
            let (name, color) = match block.speaker {
                Speaker::User => ("あなた", design::DARK.text),
                Speaker::Assistant => ("SOLO · 疑似応答", ACCENT),
                Speaker::Notice => ("状態", design::DARK.warning),
            };
            let copy = block.text.clone();
            div()
                .w_full()
                .px_6()
                .py_3()
                .child(
                    div()
                        .w_full()
                        .p_4()
                        .rounded_lg()
                        .bg(rgb(if block.speaker == Speaker::User {
                            design::DARK.elevated
                        } else {
                            PANEL
                        }))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .mb_2()
                                .text_xs()
                                .text_color(rgb(color))
                                .child(name)
                                .child(button(("copy-chat", row), "コピー").on_click(
                                    move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copy.clone(),
                                        ))
                                    },
                                )),
                        )
                        .child(
                            div()
                                .w_full()
                                .text_sm()
                                .line_height(px(23.))
                                .child(block.text.clone()),
                        ),
                )
                .into_any_element()
        })
        .size_full()
        .into_any_element()
    }

    fn logs(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = &self.sessions[self.selected];
        if session.model.logs.is_empty() {
            return empty(
                "ログはまだありません",
                "疑似ストリームを開始すると、実行状態と出力が表示されます。",
            )
            .into_any_element();
        }
        div()
            .id("log-container")
            .size_full()
            .flex()
            .flex_col()
            .on_scroll_wheel(
                cx.listener(|this, _, _, _| this.sessions[this.selected].follow_logs = false),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .px_4()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "最新 {} 件 · 省略 {} 件 · 全文 {:.2} MiB",
                        session.model.logs.len(),
                        session.model.logs_discarded,
                        session.model.log_bytes as f64 / 1048576.
                    ))
                    .child(
                        button("log-tail", "最新へ").on_click(cx.listener(|this, _, _, cx| {
                            let s = &mut this.sessions[this.selected];
                            s.follow_logs = true;
                            s.log_scroll.scroll_to_item(
                                s.model.logs.len().saturating_sub(1),
                                ScrollStrategy::Bottom,
                            );
                            cx.notify();
                        })),
                    )
                    .child(
                        button("log-path", "全文の場所をコピー").on_click(cx.listener(
                            |this, _, _, cx| {
                                let paths = this.sessions[this.selected]
                                    .artifacts
                                    .iter()
                                    .map(|path| path.display().to_string())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                cx.write_to_clipboard(ClipboardItem::new_string(paths));
                            },
                        )),
                    ),
            )
            .child(
                uniform_list(
                    "log-rows",
                    session.model.logs.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                        let s = &this.sessions[this.selected];
                        range
                            .filter_map(|i| {
                                s.model.logs.get(i).map(|row| {
                                    let text = row.text.clone();
                                    div()
                                        .id(i)
                                        .h(px(25.))
                                        .px_4()
                                        .flex()
                                        .gap_3()
                                        .text_xs()
                                        .font_family("Menlo")
                                        .overflow_hidden()
                                        .bg(rgb(if i % 2 == 0 { BG } else { PANEL }))
                                        .child(
                                            div()
                                                .w(px(68.))
                                                .flex_shrink_0()
                                                .text_color(rgb(MUTED))
                                                .child(format!("{:06}", row.sequence)),
                                        )
                                        .child(
                                            div()
                                                .w(px(44.))
                                                .flex_shrink_0()
                                                .text_color(rgb(if row.level == "event" {
                                                    design::DARK.warning
                                                } else {
                                                    ACCENT
                                                }))
                                                .child(row.level.clone()),
                                        )
                                        .child(div().truncate().child(text.clone()))
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                text.clone(),
                                            ))
                                        })
                                })
                            })
                            .collect()
                    }),
                )
                .track_scroll(session.log_scroll.clone())
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }

    fn diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.sessions[self.selected];
        let Some(diff) = s.model.diffs.get(s.diff_index) else {
            return empty(
                "差分はまだありません",
                "「会話と差分」の再生が完了すると、サンプルの変更を確認できます。",
            )
            .into_any_element();
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().flex().gap_2().px_4().py_3().children(
                s.model.diffs.iter().enumerate().map(|(i, diff)| {
                    button(("diff-file", i), diff.path.clone()).on_click(cx.listener(
                        move |this, _, _, cx| {
                            let s = &mut this.sessions[this.selected];
                            s.diff_index = i;
                            s.diff_scroll = UniformListScrollHandle::new();
                            cx.notify();
                        },
                    ))
                }),
            ))
            .when(diff.truncated, |view| {
                view.child(
                    div()
                        .px_4()
                        .text_color(rgb(design::DARK.warning))
                        .child("差分の表示上限に達しました"),
                )
            })
            .child(
                uniform_list(
                    "diff-lines",
                    diff.lines.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                        let s = &this.sessions[this.selected];
                        let diff = &s.model.diffs[s.diff_index];
                        range
                            .map(|i| {
                                let line = &diff.lines[i];
                                let (background, color) = match line.kind {
                                    DiffKind::Added => {
                                        (design::DARK.success_soft, design::DARK.success)
                                    }
                                    DiffKind::Removed => {
                                        (design::DARK.danger_soft, design::DARK.danger)
                                    }
                                    DiffKind::Header => {
                                        (design::DARK.accent_soft, design::DARK.accent_text)
                                    }
                                    DiffKind::Context => (BG, design::DARK.text),
                                };
                                let text = line.text.clone();
                                div()
                                    .id(i)
                                    .h(px(26.))
                                    .px_4()
                                    .flex()
                                    .gap_3()
                                    .font_family("Menlo")
                                    .text_xs()
                                    .bg(rgb(background))
                                    .text_color(rgb(color))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .w(px(36.))
                                            .flex_shrink_0()
                                            .text_color(rgb(MUTED))
                                            .child(
                                                line.old.map(|n| n.to_string()).unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(36.))
                                            .flex_shrink_0()
                                            .text_color(rgb(MUTED))
                                            .child(
                                                line.new.map(|n| n.to_string()).unwrap_or_default(),
                                            ),
                                    )
                                    .child(div().truncate().child(text.clone()))
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            text.clone(),
                                        ))
                                    })
                            })
                            .collect()
                    }),
                )
                .track_scroll(s.diff_scroll.clone())
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered += 1;
        let session = &self.sessions[self.selected];
        let active = session.model.status.is_active();
        let composer = session.composer.clone();
        let pane = match session.tab {
            Tab::Chat => self.conversation(cx),
            Tab::Diff => self.diff(cx),
            Tab::Logs => self.logs(cx),
        };
        design::root(cx).flex()
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|this, _: &NewSession, window, cx| { this.new_session(cx); window.focus(&this.sessions[this.selected].composer.focus_handle(cx)); }))
            .child(self.sidebar(cx))
            .child(div().flex_1().min_w_0().h_full().flex().flex_col()
                .child(div().px_5().py_3().border_b_1().border_color(rgb(BORDER)).flex().items_center().justify_between()
                    .child(div().min_w_0().child(div().font_weight(FontWeight::SEMIBOLD).child(session.model.title.clone()))
                        .child(div().mt_1().text_xs().text_color(rgb(MUTED)).truncate().child(session.model.provider.clone())))
                    .child(div().flex().gap_3().items_center()
                        .child(div().text_xs().text_color(rgb(status_color(session.model.status))).child(session.model.status.label()))
                        .child(button("close-session", "閉じる").on_click(cx.listener(|this, _, _, cx| {
                            this.sessions.remove(this.selected); this.selected = this.selected.saturating_sub(1);
                            if this.sessions.is_empty() { this.new_session(cx); } cx.notify();
                        })))))
                .child(div().flex().flex_wrap().gap_2().px_4().py_3().children([
                    Scenario::Demo, Scenario::Events10k, Scenario::Events100k, Scenario::Log100MiB, Scenario::Faults,
                ].into_iter().enumerate().map(|(i, scenario)| {
                    button(("scenario", i), scenario.label()).disabled(active)
                        .on_click(cx.listener(move |this, _, _, cx| this.scenario(scenario, cx)))
                })))
                .when(!self.message.is_empty(), |view| view.child(div().px_5().py_2().text_xs().text_color(rgb(design::DARK.warning)).child(self.message.clone())))
                .when(session.model.unknown > 0 || session.model.rejected > 0, |view| view.child(div().px_5().py_2().bg(rgb(design::DARK.warning_soft)).text_xs().text_color(rgb(design::DARK.warning))
                    .child(format!("未対応 {} 件 / 不正・順序違反 {} 件 — ログで詳細を確認できます", session.model.unknown, session.model.rejected))))
                .when(!session.model.reason.is_empty(), |view| view.child(div().px_5().py_2().text_xs().text_color(rgb(status_color(session.model.status))).child(session.model.reason.clone())))
                .child(div().flex().gap_2().px_4().pb_2().border_b_1().border_color(rgb(BORDER)).children([
                    (Tab::Chat, "会話".to_owned()), (Tab::Diff, format!("差分  {}", session.model.diffs.len())), (Tab::Logs, "ログ".to_owned()),
                ].into_iter().enumerate().map(|(i, (tab, title))| {
                    design::tab(("tab", i), title, session.tab == tab, cx)
                        .on_click(cx.listener(move |this, _, _, cx| { this.sessions[this.selected].tab = tab; cx.notify(); }))
                })))
                .child(div().flex_1().min_h_0().overflow_hidden().child(pane))
                .when(session.model.chat_discarded > 0, |view| view.child(div().px_5().py_1().text_xs().text_color(rgb(MUTED)).child(format!("表示上限のため古い会話を {} block 省略しています", session.model.chat_discarded))))
                .child(div().px_5().py_3().border_t_1().border_color(rgb(BORDER)).child(composer.clone())
                    .child(div().flex().items_center().justify_between().mt_2().text_xs().text_color(rgb(MUTED))
                        .child(if active { "受信中も入力できます。停止後に送信できます。" } else { "⌘ Enter で送信 · 日本語変換中は確定後に送信" })
                        .child(div().flex().gap_2().when(active, |view| view
                            .child(button("disconnect", "切断を試す").on_click(cx.listener(|this, _, _, _| {
                                if let Some(controller) = &this.sessions[this.selected].controller { controller.disconnect(); }
                            })))
                            .child(button("cancel", "中止").on_click(cx.listener(|this, _, _, cx| {
                                let s = &mut this.sessions[this.selected]; if let Some(controller) = &s.controller { controller.cancel(); s.model.status = Status::Cancelling; cx.notify(); }
                            }))))
                            .child(button("submit", "送信").variant(ButtonVariant::Primary).trailing_icon(design::Icon::ArrowRight).disabled(active).on_click(move |_, _, cx| composer.update(cx, |input, cx| input.submit(cx)))))))
                .child(div().h(px(26.)).px_4().flex().items_center().justify_between().text_size(px(10.)).bg(rgb(PANEL)).text_color(rgb(MUTED))
                    .child(format!("{} events · 重複 {} · 表示更新 {} 回 / 最大 {:.2} ms", session.model.accepted, session.model.duplicates, session.batches, session.max_batch_ms))
                    .child(format!("tokens {} / {} · cost {}{}", number(session.model.usage.input_tokens), number(session.model.usage.output_tokens), session.model.usage.cost_usd.map(|n| format!("${n:.4}")).unwrap_or_else(|| "不明".into()), if self.smoke { " · smoke" } else { "" }))))
    }
}

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id, label)
        .variant(ButtonVariant::Secondary)
        .control_size(ControlSize::Small)
}

fn empty(title: &'static str, subtitle: &'static str) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .justify_center()
        .items_center()
        .gap_3()
        .p_8()
        .child(div().text_xl().text_color(rgb(ACCENT)).child(title))
        .child(div().text_sm().text_color(rgb(MUTED)).child(subtitle))
}

fn status_color(status: Status) -> u32 {
    match status {
        Status::Failed | Status::Disconnected => design::DARK.danger,
        Status::Cancelling => design::DARK.warning,
        Status::Running | Status::Completed => ACCENT,
        _ => MUTED,
    }
}
fn number(value: Option<u64>) -> String {
    value
        .map(|n| n.to_string())
        .unwrap_or_else(|| "不明".into())
}
