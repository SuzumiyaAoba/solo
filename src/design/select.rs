//! Kit の検索付き Select と Solo の index ベースの実行先選択を接続する。
use gpui_kit::component::{
    IndexPath,
    select::{SearchableVec, Select as KitSelect, SelectEvent, SelectItem, SelectState},
};
use gpui_kit::{prelude::*, *};

pub struct SelectionChanged {
    pub index: usize,
    pub label: SharedString,
}
#[derive(Clone)]
struct Choice {
    index: usize,
    label: SharedString,
}
impl SelectItem for Choice {
    type Value = usize;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &usize {
        &self.index
    }
}
type Choices = SearchableVec<Choice>;

pub struct Select {
    pub options: Vec<SharedString>,
    pub selected: usize,
    pub disabled: bool,
    state: Entity<SelectState<Choices>>,
    reset: bool,
    subscription: Subscription,
}
impl EventEmitter<SelectionChanged> for Select {}
impl Focusable for Select {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.focus_handle(cx)
    }
}
impl Select {
    pub fn new(
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let options: Vec<_> = options.into_iter().map(Into::into).collect();
        let selected = selected.min(options.len().saturating_sub(1));
        let (state, subscription) = Self::build(&options, selected, window, cx);
        Self {
            options,
            selected,
            disabled: false,
            state,
            reset: false,
            subscription,
        }
    }
    fn build(
        options: &[SharedString],
        selected: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<SelectState<Choices>>, Subscription) {
        let items: Vec<_> = options
            .iter()
            .enumerate()
            .map(|(index, label)| Choice {
                index,
                label: label.clone(),
            })
            .collect();
        let selected = (!items.is_empty()).then_some(IndexPath::default().row(selected));
        let state = cx.new(|cx| {
            SelectState::new(SearchableVec::new(items), selected, window, cx).searchable(true)
        });
        let subscription = cx.subscribe(&state, |this, _, event: &SelectEvent<Choices>, cx| {
            if this.disabled {
                return;
            }
            if let SelectEvent::Confirm(Some(index)) = event {
                this.selected = *index;
                cx.emit(SelectionChanged {
                    index: *index,
                    label: this.options[*index].clone(),
                });
                cx.notify();
            }
        });
        (state, subscription)
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// 新しいセッションでは検索語と開いたメニューを持ち越さない。
    ///
    /// 再構築を描画まで遅らせると、描画が走るまでの間に差し替えられる側の
    /// フォーカスハンドルが窓に残り、先にフォーカスやキー入力が差し込まれた
    /// 場合にディスパッチ中の描画でハンドルが死んでキーが届かなくなる。
    /// エフェクトの区切りでウィンドウが取れる限り、ここで先に再構築する。
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.reset {
            cx.notify();
            return;
        }
        self.reset = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.update(|app| {
                app.with_window(this.entity_id(), |window, app| {
                    let _ = this.update(app, |this, cx| this.rebuild_if_reset(window, cx));
                });
            });
        })
        .detach();
    }
    fn rebuild_if_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.reset {
            return false;
        }
        (self.state, self.subscription) = Self::build(&self.options, self.selected, window, cx);
        self.reset = false;
        true
    }
}
impl Render for Select {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rebuild_if_reset(window, cx);
        if self.state.read(cx).selected_value().copied()
            != self.options.get(self.selected).map(|_| self.selected)
        {
            let selected = self
                .options
                .get(self.selected)
                .map(|_| IndexPath::default().row(self.selected));
            self.state.update(cx, |state, cx| {
                state.set_selected_index(selected, window, cx)
            });
        }
        KitSelect::new(&self.state)
            .w_full()
            .menu_width(px(300.))
            .disabled(self.disabled || self.options.is_empty())
            .placeholder("選択肢がありません")
            .search_placeholder("実行先・項目を検索…")
            .accessibility_label("実行先・項目の選択")
    }
}
