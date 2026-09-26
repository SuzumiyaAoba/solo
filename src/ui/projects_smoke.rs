use super::projects::ProjectManager;
use super::*;
use gpui_kit::component::WindowExt;
use std::{fs, time::Duration};

pub(super) fn start(window: &Window, cx: &mut Context<ProjectManager>) {
    cx.spawn_in(window, async move |this, cx| {
        let (original, a, b, path_a, path_b) = this.update_in(cx, |this, window, cx| {
            let original = this.catalog.active.expect("initial project");
            let dir = this.temporary.as_ref().expect("isolated project store").path();
            let path_a = dir.join("alpha");
            let path_b = dir.join("beta");
            for (path, name) in [(&path_a, "Alpha agent"), (&path_b, "Beta agent")] {
                fs::create_dir_all(path.join(".solo")).unwrap();
                fs::write(path.join(".solo/agents.json"), serde_json::json!({ "agents": [{
                    "id": "agent", "name": name, "command": "sh", "args": []
                }]}).to_string()).unwrap();
                fs::write(path.join("keep.txt"), "project data").unwrap();
            }
            let path_a = path_a.canonicalize().unwrap();
            let path_b = path_b.canonicalize().unwrap();
            let a = this.add_project(&path_a, window, cx).unwrap();
            let alpha = this.workspace(a).unwrap();
            alpha.update(cx, |workspace, cx| {
                assert_eq!(workspace.acp_agents[0].name, "Alpha agent");
                assert_eq!(workspace.command_rules.as_ref().unwrap().workspace(), path_a.canonicalize().unwrap());
                workspace.sessions[0].composer.update(cx, |input, cx| input.set_value("Alpha の下書き🙂", cx));
                workspace.show_tab(Tab::Logs, cx);
                workspace.start_mock(0, Scenario::Demo, "Alpha のデモ".into(), cx);
            });
            let b = this.add_project(&path_b, window, cx).unwrap();
            this.workspace(b).unwrap().update(cx, |workspace, cx| {
                assert_eq!(workspace.acp_agents[0].name, "Beta agent");
                assert_eq!(workspace.command_rules.as_ref().unwrap().workspace(), path_b.canonicalize().unwrap());
                workspace.new_session(window, cx);
                workspace.sessions[1].composer.update(cx, |input, cx| input.set_value("Beta の下書き🧪", cx));
                workspace.start_mock(1, Scenario::Demo, "Beta のデモ".into(), cx);
            });
            assert!(!alpha.read(cx).is_visible);
            assert_eq!(alpha.read(cx).selected, 0);
            assert!(this.rename_project(a, "設計プロジェクト", window, cx));
            assert_eq!(this.catalog.active, Some(b));
            assert_eq!(alpha.read(cx).workspace_name, "設計プロジェクト");
            assert!(!this.remove_project(a, window, cx), "running project was removed");
            this.error = None;
            (original, a, b, path_a, path_b)
        }).unwrap();
        let started = Instant::now();
        loop {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let done = this.update_in(cx, |this, _, cx| {
                !this.workspace(a).unwrap().read(cx).sessions[0].model.status.is_active()
                    && !this.workspace(b).unwrap().read(cx).sessions[1].model.status.is_active()
            }).unwrap();
            if done { break; }
            assert!(started.elapsed() < Duration::from_secs(10), "project workers timed out");
        }
        this.update_in(cx, |this, window, cx| {
            let alpha = this.workspace(a).unwrap();
            let beta = this.workspace(b).unwrap();
            assert_eq!(alpha.read(cx).sessions[0].model.workspace, path_a.display().to_string());
            assert_eq!(beta.read(cx).sessions[1].model.workspace, path_b.display().to_string());
            assert_eq!(alpha.read(cx).sessions[0].model.title, "Alpha のデモ");
            assert_eq!(beta.read(cx).sessions[1].model.title, "Beta のデモ");
            assert!(beta.read(cx).other_project_attention > 0);
            assert!(this.select_project(a, window, cx));
            assert!(alpha.read(cx).sessions[0].tab == Tab::Logs);
            assert_eq!(alpha.read(cx).sessions[0].composer.read(cx).value(cx), "Alpha の下書き🙂");
            assert!(!beta.read(cx).is_visible);
            let beta_channel = beta.read(cx).sessions[1].model.id.clone();
            this.select_channel(b, &beta_channel, window, cx);
            assert_eq!(this.catalog.active, Some(b));
            assert_eq!(beta.read(cx).selected, 1);
            assert_eq!(beta.read(cx).sessions[1].composer.read(cx).value(cx), "Beta の下書き🧪");
            // フォルダが移動・削除されても、既に開いたセッションへのアクセスを失わない。
            let missing = path_a.parent().unwrap().join("removed-folder");
            fs::create_dir(&missing).unwrap();
            let missing_id = this.add_project(&missing, window, cx).unwrap();
            let cached = this.workspace(missing_id).unwrap();
            fs::remove_dir(&missing).unwrap();
            assert!(this.select_project(b, window, cx));
            assert!(this.select_project(missing_id, window, cx));
            assert_eq!(this.workspace(missing_id).unwrap().entity_id(), cached.entity_id());
            assert!(this.remove_project(missing_id, window, cx));
            let count = this.catalog.projects.len();
            assert_eq!(this.add_project(&path_a, window, cx), Some(a));
            assert_eq!(this.catalog.projects.len(), count, "duplicate folder created another project");
            assert_eq!(this.workspace(a).unwrap().entity_id(), alpha.entity_id());

            // プロジェクト A の待機状態・下書きを B に混ぜない。実モデルは起動しない。
            alpha.update(cx, |workspace, cx| {
                workspace.new_session(window, cx);
                workspace.sessions[1].backend = Some(Backend::Subscription);
                workspace.sessions[1].model.status = Status::Running;
                workspace.new_session(window, cx);
                workspace.start_selected(2, "Alpha の順番待ち".into(), cx);
                assert_eq!(workspace.queue.len(), 1);
            });
            assert!(beta.read(cx).queue.is_empty());
            assert!(!this.remove_project(a, window, cx));
            alpha.update(cx, |workspace, cx| {
                workspace.cancel_queued(cx);
                assert_eq!(workspace.sessions[2].composer.read(cx).value(cx), "Alpha の順番待ち");
                workspace.close_session(window, cx);
                workspace.sessions[1].model.status = Status::Idle;
                workspace.close_session(window, cx);
            });
            this.error = None;
            let saved = this.store.as_ref().unwrap().load().unwrap();
            assert_eq!(saved.active, Some(a));
            assert_eq!(saved.get(a).unwrap().name, "設計プロジェクト");
        }).unwrap();
        cx.update(|window, cx| { window.dispatch_keystroke(Keystroke::parse("cmd-shift-p").unwrap(), cx); }).unwrap();
        cx.background_executor().timer(Duration::from_millis(100)).await;
        cx.update(|window, cx| assert!(window.has_active_sheet(cx), "project shortcut did not open the picker")).unwrap();
        for (scheme, width, height) in [(ColorScheme::Dark, 1240., 840.), (ColorScheme::Light, 820., 620.)] {
            this.update_in(cx, |_, window, cx| { ds::set_theme(scheme, cx); window.resize(size(px(width), px(height))); }).unwrap();
            println!("Projects UI ready: {} / {}", scheme.label(), width);
            let pause = std::env::var("SOLO_PROJECT_PREVIEW_MS").ok().and_then(|value| value.parse::<u64>().ok()).unwrap_or(180).min(30_000);
            cx.background_executor().timer(Duration::from_millis(pause)).await;
        }
        let original_path = this.update_in(cx, |this, window, cx| {
            assert!(this.open_project(b, window, cx));
            assert!(!window.has_active_sheet(cx));
            let beta = this.workspace(b).unwrap();
            assert!(beta.read(cx).sessions[1].composer.focus_handle(cx).is_focused(window), "project switch restored focus into the old project");
            assert!(this.remove_project(a, window, cx));
            assert!(path_a.join("keep.txt").is_file(), "removal deleted project files");
            assert_eq!(this.workspace(b).unwrap().read(cx).sessions[1].composer.read(cx).value(cx), "Beta の下書き🧪");
            assert!(this.remove_project(b, window, cx));
            assert!(path_b.join("keep.txt").is_file());
            assert!(this.select_project(original, window, cx));
            assert_eq!(this.catalog.projects.len(), 1);
            let path = this.catalog.get(original).unwrap().path.clone();
            assert!(this.remove_project(original, window, cx));
            assert!(this.catalog.active.is_none());
            assert!(this.store.as_ref().unwrap().load().unwrap().projects.is_empty());
            path
        }).unwrap();
        cx.update(|window, cx| { window.dispatch_keystroke(Keystroke::parse("cmd-n").unwrap(), cx); }).unwrap();
        cx.background_executor().timer(Duration::from_millis(180)).await;
        this.update_in(cx, |this, window, cx| {
            assert!(this.catalog.projects.is_empty(), "empty project state created an unscoped session");
            let restored = this.add_project(&original_path, window, cx).unwrap();
            println!("Project smoke OK: folders, rename, persistence, duplicate selection, isolated drafts/backends/rules/queues/events, background attention, removal guards, empty state, input focus, both themes and compact layout");
            // 続けて既存の全セッション・承認・ストリーム検証を行う。
            window.resize(size(px(1240.), px(840.)));
            this.workspace(restored).unwrap().update(cx, |workspace, cx| {
                workspace.start_mock(0, Scenario::Demo, "Phase 0 の表示を確認してください。".into(), cx);
                super::smoke::start(window, cx);
            });
        }).unwrap();
    }).detach();
}
