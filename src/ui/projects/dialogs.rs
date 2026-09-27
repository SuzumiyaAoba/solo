use super::*;
use gpui_kit::component::{WindowExt, dialog::DialogButtonProps};

impl ProjectManager {
    pub(super) fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.choosing_folder {
            return;
        }
        self.choosing_folder = true;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("プロジェクトを追加".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = choice.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.choosing_folder = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first()
                            && this.add_project(path, window, cx).is_some()
                        {
                            window.close_sheet(cx);
                            this.focus_active(window, cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => this.fail(error, cx),
                    Err(error) => this.fail(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(in crate::ui) fn rename_project(
        &mut self,
        id: u64,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut next = self.catalog.clone();
        if let Err(error) = next.rename(id, name) {
            self.fail(error, cx);
            return false;
        }
        if !self.commit(next, cx) {
            return false;
        }
        self.activate(window, cx);
        true
    }

    pub(super) fn edit_name(&self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.catalog.get(id) else {
            return;
        };
        let name = project.name.clone();
        let draft = cx.new(|cx| {
            Composer::new(window, cx)
                .default_value(&name)
                .placeholder("プロジェクト名")
        });
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let draft = draft.clone();
            let weak = weak.clone();
            dialog
                .title("プロジェクト名")
                .w(px(420.))
                .child(draft.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("保存")
                        .cancel_text("戻る")
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    let name = draft.read(cx).value(cx).to_string();
                    weak.update(cx, |this, cx| this.rename_project(id, &name, window, cx))
                        .unwrap_or(false)
                })
        });
    }

    pub(in crate::ui) fn remove_project(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.activity(id, cx).busy() {
            self.fail("実行・順番待ちの終了後に登録を解除できます", cx);
            return false;
        }
        let mut next = self.catalog.clone();
        if let Err(error) = next.remove(id) {
            self.fail(error, cx);
            return false;
        }
        if !self.commit(next, cx) {
            return false;
        }
        self.opened.retain(|opened| opened.id != id);
        self.activate(window, cx);
        true
    }

    pub(super) fn confirm_remove(&self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.catalog.get(id).cloned() else {
            return;
        };
        let activity = self.activity(id, cx);
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let weak = weak.clone();
            let p = ds::theme(cx);
            dialog
                .title("プロジェクトの登録解除")
                .w(px(440.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(project.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::Close.view(p.warning))
                        .child(format!("セッション {} 件を閉じる", activity.sessions)),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(p.muted))
                        .child(project.path.display().to_string()),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("登録解除")
                        .cancel_text("戻る")
                        .show_cancel(true)
                        .ok_variant(gpui_kit::component::button::ButtonVariant::Danger),
                )
                .on_ok(move |_, window, cx| {
                    weak.update(cx, |this, cx| this.remove_project(id, window, cx))
                        .unwrap_or(false)
                })
        });
    }
}
