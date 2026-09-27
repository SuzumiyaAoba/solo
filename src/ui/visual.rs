//! `--visual DIR` 用の実描画キャプチャ。feature `gui-visual` でのみ有効。
//! 実ウィンドウを開かず Metal でオフスクリーン描画し、PNG を DIR へ出力する。
//! メインスレッド必須（`MacPlatform`）。`cargo run --features gui-visual -- --visual /tmp/solo`。
use super::*;
use gpui_kit::component::WindowExt;
use gpui_kit::{HeadlessAppContext, WindowHandle};
use std::{path::Path, sync::Arc};

pub(super) fn run(dir: &Path) {
    std::fs::create_dir_all(dir).expect("visual output dir");
    let platform = gpui_kit::platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(DesignAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(ds::init);
    for scheme in [ColorScheme::Dark, ColorScheme::Light] {
        cx.update(|cx| ds::set_theme(scheme, cx));
        let name = scheme.label().to_lowercase();
        let window = idle_window(&mut cx);
        capture(&mut cx, window, dir, &format!("composer-{name}-idle"));

        let window = idle_window(&mut cx);
        window
            .update(&mut cx, |workspace, window, cx| {
                let composer = workspace.sessions[workspace.selected].composer.clone();
                composer.update(cx, |input, cx| {
                    input.set_value("差分の作り込みを続けてください", cx)
                });
                window.refresh();
            })
            .unwrap();
        capture(&mut cx, window, dir, &format!("composer-{name}-filled"));
        let window = idle_window(&mut cx);
        window
            .update(&mut cx, |workspace, window, _cx| {
                workspace.sessions[workspace.selected].model.status = Status::Running;
                window.refresh();
            })
            .unwrap();
        capture(&mut cx, window, dir, &format!("composer-{name}-running"));

        // mock backend の実行中は disconnect + 中止の2ボタンになる。
        let window = idle_window(&mut cx);
        window
            .update(&mut cx, |workspace, window, cx| {
                workspace.start_mock(
                    workspace.selected,
                    Scenario::Events100k,
                    "ストリーム中の表示確認".into(),
                    cx,
                );
                window.refresh();
            })
            .unwrap();
        capture(&mut cx, window, dir, &format!("composer-{name}-mock"));

        let manager_slot = std::rc::Rc::new(std::cell::RefCell::new(None));
        let slot = manager_slot.clone();
        let window = cx
            .open_window(size(px(1240.), px(840.)), move |window, cx| {
                let view = cx.new(|cx| projects::ProjectManager::new(true, window, cx));
                *slot.borrow_mut() = Some(view.clone());
                cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
            })
            .expect("open manager window");

        // ルール管理シートのツールタブ(Allow/Deny の既定と選択肢)。
        cx.update_window(window.into(), |_, window, cx| {
            let manager = manager_slot.borrow().clone().expect("manager entity");
            manager.update(cx, |manager, cx| {
                let active = manager
                    .catalog
                    .active
                    .expect("visual manager has an active project");
                manager
                    .workspace(active)
                    .expect("active workspace is open")
                    .update(cx, |workspace, cx| {
                        workspace.open_command_rules(window, cx);
                        workspace.rule_editor.update(cx, |editor, cx| {
                            editor.page = command_rules::Page::Tools;
                            cx.notify();
                        });
                    });
            });
        })
        .unwrap();
        capture(&mut cx, window, dir, &format!("tools-{name}"));

        cx.update_window(window.into(), |_, window, cx| window.close_sheet(cx))
            .unwrap();
        capture(&mut cx, window, dir, &format!("sidebar-{name}"));
    }
}

/// 一時 workspace の Workspace だけを描く headless ウィンドウ。
fn idle_window(cx: &mut HeadlessAppContext) -> WindowHandle<Workspace> {
    let dir = tempfile::tempdir().expect("visual workspace");
    let path = dir.path().to_path_buf();
    std::mem::forget(dir); // プロセス終了まで保持
    cx.open_window(size(px(1240.), px(840.)), move |window, cx| {
        let toast = cx.new(ToastHost::new);
        cx.new(|cx| Workspace::new(path.clone(), "visual".into(), true, toast, window, cx))
    })
    .expect("open headless window")
}

fn capture<T: 'static>(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<T>,
    dir: &Path,
    name: &str,
) {
    cx.run_until_parked();
    let image = cx
        .capture_screenshot(window.into())
        .expect("headless renderer must produce a screenshot");
    let path = dir.join(format!("{name}.png"));
    image.save(&path).expect("write screenshot");
    println!(
        "visual: {} -> {} ({}x{})",
        name,
        path.display(),
        image.width(),
        image.height()
    );
}
