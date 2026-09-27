use super::*;
use solo::projects::MAX_PROJECTS;

impl ProjectManager {
    pub(super) fn show_projects(&self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity();
        let panel = cx.new(|cx| {
            let observation = cx.observe(&owner, |_, _, cx| cx.notify());
            ProjectPanel {
                owner: owner.downgrade(),
                _observation: observation,
            }
        });
        let weak = cx.weak_entity();
        window.open_sheet(cx, move |sheet, _, _| {
            let weak = weak.clone();
            sheet
                .title("プロジェクト")
                .size(px(400.))
                .child(panel.clone())
                .on_close(move |_, window, cx| {
                    let weak = weak.clone();
                    window.defer(cx, move |window, cx| {
                        let _ = weak.update(cx, |this, cx| this.focus_active(window, cx));
                    });
                })
        });
    }
}

struct ProjectPanel {
    owner: WeakEntity<ProjectManager>,
    _observation: Subscription,
}
impl Render for ProjectPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(owner) = self.owner.upgrade() else {
            return div().into_any_element();
        };
        let (rows, active, error, choosing) = {
            let state = owner.read(cx);
            (
                state
                    .catalog
                    .projects
                    .iter()
                    .map(|project| (project.clone(), state.activity(project.id, cx)))
                    .collect::<Vec<_>>(),
                state.catalog.active,
                state.error.clone(),
                state.choosing_folder,
            )
        };
        let p = ds::theme(cx);
        let weak = self.owner.clone();
        let reload = weak.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.muted))
                            .child(format!("{} / {MAX_PROJECTS}", rows.len())),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                Button::icon(
                                    "reload-projects",
                                    Icon::RotateCcw,
                                    "プロジェクト一覧を再読み込み",
                                )
                                .on_click(move |_, window, cx| {
                                    let _ = reload.update(cx, |this, cx| this.reload(window, cx));
                                }),
                            )
                            .child(
                                Button::icon(
                                    "add-project",
                                    Icon::Plus,
                                    "フォルダをプロジェクトに追加 · ⌘ ⇧ O",
                                )
                                .variant(ButtonVariant::Primary)
                                .disabled(choosing || rows.len() >= MAX_PROJECTS)
                                .on_click(move |_, window, cx| {
                                    let _ =
                                        weak.update(cx, |this, cx| this.choose_folder(window, cx));
                                }),
                            ),
                    ),
            )
            .when_some(error, |v, error| {
                v.child(ds::alert("プロジェクト設定", error, Tone::Warning, cx))
            })
            .child(
                div()
                    .id("project-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(rows.into_iter().map(|(project, activity)| {
                        let id = project.id;
                        let select = self.owner.clone();
                        let rename = self.owner.clone();
                        let remove = self.owner.clone();
                        ds::card(cx)
                            .p_3()
                            .gap_2()
                            .when(active == Some(id), |v| v.border_color(rgb(p.accent)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new(
                                            ("open-project", id as usize),
                                            project.name.clone(),
                                        )
                                        .with_icon(Icon::Folder)
                                        .variant(ButtonVariant::Ghost)
                                        .toggled(active == Some(id))
                                        .min_w_0()
                                        .max_w(px(260.))
                                        .justify_start()
                                        .tooltip(project.path.display().to_string())
                                        .on_click(
                                            move |_, window, cx| {
                                                let _ = select.update(cx, |this, cx| {
                                                    this.open_project(id, window, cx);
                                                });
                                            },
                                        ),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        Button::icon(
                                            ("rename-project", id as usize),
                                            Icon::Pencil,
                                            "プロジェクト名を変更",
                                        )
                                        .control_size(ControlSize::Small)
                                        .on_click(
                                            move |_, window, cx| {
                                                let _ = rename.update(cx, |this, cx| {
                                                    this.edit_name(id, window, cx)
                                                });
                                            },
                                        ),
                                    )
                                    .child(
                                        Button::icon(
                                            ("remove-project", id as usize),
                                            Icon::Close,
                                            "プロジェクトの登録解除",
                                        )
                                        .control_size(ControlSize::Small)
                                        .disabled(activity.busy())
                                        .when(activity.busy(), |b| {
                                            b.tooltip("実行・順番待ちの終了後に登録解除")
                                        })
                                        .on_click(
                                            move |_, window, cx| {
                                                let _ = remove.update(cx, |this, cx| {
                                                    this.confirm_remove(id, window, cx)
                                                });
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(rgb(p.muted))
                                    .child(project.path.display().to_string()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(ds::indicator(
                                        ("project-sessions", id as usize),
                                        Icon::MessageSquare,
                                        activity.sessions.to_string(),
                                        format!("セッション {} 件", activity.sessions),
                                        Tone::Neutral,
                                        cx,
                                    ))
                                    .when(activity.running > 0, |v| {
                                        v.child(ds::indicator(
                                            ("project-running", id as usize),
                                            Icon::Play,
                                            activity.running.to_string(),
                                            "実行中",
                                            Tone::Accent,
                                            cx,
                                        ))
                                    })
                                    .when(activity.queued > 0, |v| {
                                        v.child(ds::indicator(
                                            ("project-queued", id as usize),
                                            Icon::Clock,
                                            activity.queued.to_string(),
                                            "順番待ち",
                                            Tone::Neutral,
                                            cx,
                                        ))
                                    })
                                    .when(activity.attention > 0, |v| {
                                        v.child(ds::indicator(
                                            ("project-attention", id as usize),
                                            Icon::Bell,
                                            activity.attention.to_string(),
                                            "要対応",
                                            Tone::Warning,
                                            cx,
                                        ))
                                    }),
                            )
                    })),
            )
            .into_any_element()
    }
}
