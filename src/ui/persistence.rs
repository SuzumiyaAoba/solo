//! セッションのイベント・属性・順番待ちを WorkspaceStore へ保存し、起動時に復元する。
use super::*;
use std::time::Duration;

/// 下書き保存のデバウンス間隔。キー入力ごとの FileTransaction を避ける。
const META_SAVE_DELAY: Duration = Duration::from_millis(600);

impl SessionView {
    /// 保存ファイルを open、無ければ create で採番を確定する。open が
    /// NotFound 以外で失敗した場合は上書きせずエラーとする(破損ファイルの保護)。
    /// create で ID が衝突して変わった場合は model/meta を合わせて差し替える。
    pub(super) fn open_or_create(
        &mut self,
        store: &WorkspaceStore,
    ) -> std::io::Result<&mut SessionFile> {
        if self.file.is_none() {
            self.file = Some(match store.open(&self.model.id) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let file = store.create(&self.model.id)?;
                    let id = file.session_id().to_owned();
                    if id != self.model.id {
                        self.model.id = id.clone();
                        self.meta.session_id = id;
                    }
                    file
                }
                Err(error) => return Err(error),
            });
        }
        Ok(self.file.as_mut().expect("file just opened"))
    }

    /// 受信イベントを追記する。失敗は persist_error に畳み込み、呼出側で一度だけ通知する。
    pub(super) fn persist_event(
        &mut self,
        store: &WorkspaceStore,
        envelope: &solo::event::Envelope,
    ) {
        if let Err(error) = self
            .open_or_create(store)
            .and_then(|file| file.append(envelope))
        {
            self.persist_error
                .get_or_insert_with(|| format!("セッションを保存できません: {error}"));
        }
    }

    /// meta のイベント由来フィールドを現在の model から写す(draft と reviewed 以外)。
    pub(super) fn sync_meta(&mut self) {
        self.meta.title = self.model.title.clone();
        self.meta.provider = self.model.provider.clone();
        self.meta.unread_result = self.unread_result;
        self.meta.store_capped = self.file.as_ref().is_some_and(|file| file.capped());
    }

    /// meta を保存。file がまだ無ければ作成し、draft を呼出側の現在値に同期して書く。
    pub(super) fn save_meta(&mut self, store: &WorkspaceStore, draft: &str) -> std::io::Result<()> {
        self.meta.draft = draft.to_owned();
        self.sync_meta();
        // open_or_create の &mut self と meta の &self が競合しないよう先に複製する。
        let meta = self.meta.clone();
        self.open_or_create(store)?.save_meta(&meta)
    }

    /// 実行履歴(harness::Message)を別ファイルで保存。Subscription 継続実行用。
    pub(super) fn save_history(&mut self, store: &WorkspaceStore) {
        if self.history.is_empty() {
            return;
        }
        let history = self.history.clone();
        if let Err(error) = self
            .open_or_create(store)
            .and_then(|file| file.save_history(&history))
        {
            self.persist_error
                .get_or_insert_with(|| format!("会話履歴を保存できません: {error}"));
        }
    }
}

impl Workspace {
    /// 順番待ちに積まれた「選択 index」を保存用の識別へ写す。実行中 backend ではなく
    /// enqueue 時点の選択が対象(未実行なので backend はまだ確定していない)。
    pub(super) fn backend_kind_at(&self, index: usize) -> Option<BackendKind> {
        if index == 0 {
            Some(BackendKind::Subscription)
        } else if let Some(agent) = self.acp_agents.get(index - 1) {
            Some(BackendKind::Acp {
                id: agent.id.clone(),
            })
        } else {
            SCENARIOS
                .iter()
                .enumerate()
                .nth(index.wrapping_sub(1 + self.acp_agents.len()))
                .map(|(scenario, _)| BackendKind::Mock { scenario })
        }
    }

    /// 保存した実行先を現在の選択リストの index へ戻す。ACP agent が消えた場合は None。
    fn backend_index(&self, backend: &BackendKind) -> Option<usize> {
        match backend {
            BackendKind::Subscription => Some(0),
            BackendKind::Acp { id } => self
                .acp_agents
                .iter()
                .position(|agent| &agent.id == id)
                .map(|index| index + 1),
            BackendKind::Mock { scenario } => SCENARIOS
                .get(*scenario)
                .map(|_| 1 + self.acp_agents.len() + scenario),
        }
    }
    /// イベント履歴と会話の書き出し。保存先のパスは通知とクリップボードへ。
    pub(super) fn export_session(&mut self, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            self.message = "保存領域がないため書き出せません".into();
            cx.notify();
            return;
        };
        let session = &self.sessions[self.selected];
        match solo::session_store::exports_root()
            .and_then(|root| store.export(&session.model, &root))
        {
            Ok(dir) => {
                self.copy(
                    dir.display().to_string(),
                    "会話とイベントを書き出しました(パスをコピー済み)",
                    cx,
                );
            }
            Err(error) => {
                self.message = format!("書き出しに失敗しました: {error}");
                cx.notify();
            }
        }
    }

    /// セッションの meta を現在の表示状態で保存する。失敗は message に畳み込む。
    pub(super) fn save_session_meta(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        let session = &mut self.sessions[index];
        let draft = session.composer.read(cx).value(cx).to_string();
        if let Err(error) = session.save_meta(&store, &draft) {
            session
                .persist_error
                .get_or_insert_with(|| format!("セッション状態を保存できません: {error}"));
        }
        self.drain_persist_error(index);
    }

    /// 入力中の下書きを遅延保存する。連続入力では最後の世代だけが書き込む。
    pub(super) fn schedule_meta_save(&mut self, index: usize, cx: &mut Context<Self>) {
        let session = &mut self.sessions[index];
        session.meta_save_gen += 1;
        let generation = session.meta_save_gen;
        let id = session.model.id.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(META_SAVE_DELAY).await;
            this.update(cx, |this, cx| {
                let Some(index) = this.session_index(&id) else {
                    return;
                };
                if this.sessions[index].meta_save_gen == generation {
                    this.save_session_meta(index, cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// workspace.json(選択中セッションと順番待ち)を保存する。
    pub(super) fn save_workspace_state(&mut self, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        let entries = self
            .queue
            .entries()
            .filter(|run| self.session_index(&run.session_id).is_some())
            .filter_map(|run| {
                self.backend_kind_at(run.backend_index)
                    .map(|backend| QueueEntry {
                        session_id: run.session_id.clone(),
                        backend,
                        prompt: run.prompt.clone(),
                    })
            })
            .collect();
        let state = WorkspaceState {
            version: SCHEMA_VERSION,
            selected: self
                .sessions
                .get(self.selected)
                .map(|session| session.model.id.clone()),
            queue: StoredQueue {
                paused: self.queue.paused,
                entries,
            },
        };
        if let Err(error) = store.save_workspace_state(&state) {
            self.report(format!("順番待ちを保存できません: {error}"));
        }
        cx.notify();
    }

    /// 起動時に保存済みセッションを読み、順番待ちと選択を復元する。
    /// 途中で止まった実行は「結果未確認」の Disconnected として戻り、自動実行は止めたまま。
    pub(super) fn restore_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        if let Err(error) = store.sweep_orphans() {
            self.report(format!("保存済みセッションの掃除に失敗しました: {error}"));
        }
        let state = match store.load_workspace_state() {
            Ok(state) => state,
            Err(error) => {
                self.report(format!("ワークスペース状態を読み込めません: {error}"));
                WorkspaceState::default()
            }
        };
        let mut restored = Vec::new();
        let mut corrupt = 0usize;
        for id in store.session_ids().unwrap_or_default() {
            match store.load(&id) {
                Ok(item) => restored.push(item),
                Err(error) => {
                    eprintln!("solo: セッション {id} を読み込めませんでした: {error}");
                    corrupt += 1;
                }
            }
        }
        if corrupt > 0 {
            self.report(format!(
                "保存済みセッション {corrupt} 件を読み込めませんでした。ログを確認してください"
            ));
        }
        restored.sort_by_key(|item| item.meta.as_ref().map(|meta| meta.serial).unwrap_or(0));
        let mut interrupted_real = false;
        for item in restored {
            interrupted_real |= item.interrupted
                && !matches!(
                    item.meta.as_ref().and_then(|meta| meta.backend.as_ref()),
                    Some(BackendKind::Mock { .. })
                );
            self.mount_restored(item, window, cx);
        }
        self.restore_queue(&state, interrupted_real, cx);
        if let Some(selected) = state
            .selected
            .and_then(|id| self.session_index(&id))
            .or((!self.sessions.is_empty()).then_some(0))
        {
            self.selected = selected;
        }
        self.serial = self
            .sessions
            .iter()
            .map(|session| session.meta.serial)
            .max()
            .unwrap_or(0);
    }

    /// 保存された順番待ちを現行セッションへ対応付けて戻す。
    /// 失敗・切断・中断が残っているか、一時停止中だったキューは明示的な再開まで動かさない。
    fn restore_queue(&mut self, state: &WorkspaceState, interrupted_real: bool, cx: &mut App) {
        let requested = state.queue.entries.len();
        let paused = state.queue.paused || interrupted_real || requested > 0;
        let entries = state
            .queue
            .entries
            .iter()
            .filter_map(|entry| {
                let index = self.session_index(&entry.session_id)?;
                let backend_index = self.backend_index(&entry.backend)?;
                let session = &mut self.sessions[index];
                // キュー待ちの見た目を復元: 依頼文を入力欄へ戻し、変更不可にする。
                session.composer.update(cx, |input, cx| {
                    input.set_value(entry.prompt.clone(), cx);
                    input.can_submit = false;
                    input.read_only = true;
                });
                Some(QueuedRun {
                    session_id: entry.session_id.clone(),
                    backend_index,
                    prompt: entry.prompt.clone(),
                })
            })
            .collect::<Vec<_>>();
        if requested != entries.len() {
            self.report("消えた実行先やセッションを参照する順番待ちを復元できませんでした".into());
        }
        self.queue.restore(entries, paused);
    }

    /// 保存済みセッションから SessionView を組み立てる。実行中だったものは mark_recovered 済み。
    pub(super) fn mount_restored(
        &mut self,
        item: RestoredSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let RestoredSession {
            session: model,
            meta,
            history,
            interrupted: _, // store.load 時点で mark_recovered 済み
            corrupted_lines,
            truncated_tail,
            log_paths,
        } = item;
        let meta = meta.unwrap_or_else(|| SessionMeta {
            session_id: model.id.clone(),
            title: model.title.clone(),
            ..SessionMeta::default()
        });
        // 実行中 backend は表示上の選択から復元する。Mock 番号が範囲外なら実行先なし。
        let backend = meta.backend.as_ref().and_then(Backend::from_kind);
        let selected_backend = meta
            .backend
            .as_ref()
            .and_then(|kind| self.backend_index(kind))
            .unwrap_or(0);
        let mut view = self.build_session_view(model.id.clone(), window, cx);
        view.model = model;
        // consume の splice 前提に合わせて、復元済み会話の行数を scroller へ登録する。
        let chat_len = view.model.chat.len();
        view.chat_list.update(cx, |list, cx| {
            list.splice(0..0, chat_len, cx);
        });
        view.meta = meta;
        view.history = history;
        view.backend = backend;
        view.selected_backend = selected_backend;
        view.unread_result = view.meta.unread_result;
        view.artifacts = log_paths.into_iter().map(Arc::new).collect();
        for path in &view.meta.reviewed {
            if let Some(diff) = view.model.diffs.iter_mut().find(|diff| &diff.path == path) {
                diff.reviewed = true;
            }
        }
        if !view.meta.draft.is_empty() {
            let draft = view.meta.draft.clone();
            view.composer.update(cx, |input, cx| {
                input.set_value(draft, cx);
            });
        }
        if corrupted_lines > 0 || truncated_tail {
            view.model.flag_incomplete(format!(
                "保存されたイベントの一部が壊れています(破損 {corrupted_lines} 行{})",
                if truncated_tail {
                    "・末尾途中"
                } else {
                    ""
                }
            ));
        }
        // 復元では file を遅延 open にする: 読み取りだけでディスクを触らない。
        self.sessions.push(view);
    }

    /// 新規セッションのストアを確保し、ID・meta を確定する。失敗時は保存なしで続行する。
    pub(super) fn provision_session(
        &mut self,
        serial: u64,
    ) -> (String, Option<SessionFile>, SessionMeta) {
        let preferred = format!("session-{serial}");
        let mut meta = SessionMeta {
            serial,
            ..SessionMeta::default()
        };
        let Some(store) = &self.store else {
            meta.session_id = preferred.clone();
            return (preferred, None, meta);
        };
        match store.create(&preferred) {
            Ok(file) => {
                let id = file.session_id().to_owned();
                meta.session_id = id.clone();
                (id, Some(file), meta)
            }
            Err(error) => {
                self.report(format!("セッションを保存できません: {error}"));
                meta.session_id = preferred.clone();
                (preferred, None, meta)
            }
        }
    }
}
