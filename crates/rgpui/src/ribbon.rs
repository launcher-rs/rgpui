//! Ribbon 功能区 —— 数据驱动的套件风格功能区。
//!
//! 命令项按 **命令 id** 绑定到 [`CommandRegistry`](crate::CommandRegistry)：
//! 标签/快捷键未显式指定时从注册表读取，点击时经
//! [`App::dispatch_command`](crate::App::dispatch_command) 派发对应动作。
//! 因此同一个功能区既可驱动 UI，也可被快捷键、命令面板、CLI、MCP 复用。
//!
//! ## 结构
//!
//! `Ribbon`（[`RibbonState`] 实体 + 若干 [`RibbonTab`]）
//!  → `RibbonTab`（上下文选项卡可选 + 若干 [`RibbonGroup`]）
//!  → `RibbonGroup`（若干 [`RibbonItem`]）
//!  → [`RibbonItem`]：大按钮 / 小按钮 / 切换按钮 / 下拉按钮 / 分隔符。
//!
//! ## 用法
//!
//! ```ignore
//! use rgpui::ribbon::{Ribbon, RibbonItem, RibbonGroup, RibbonTab, RibbonState};
//!
//! let state = cx.new(RibbonState::new);
//! // 父视图应 observe(state) 以便切换选项卡时重绘。
//!
//! Ribbon::new(&state, vec![
//!     RibbonTab::new("home", "开始").group(
//!         RibbonGroup::new("剪贴板")
//!             .item(RibbonItem::button("app::Paste").label("粘贴").large())
//!             .item(RibbonItem::button("app::Cut").label("剪切"))
//!             .item(RibbonItem::button("app::Copy").label("复制")),
//!     ),
//! ])
//! ```
//!
//! 下拉按钮依赖菜单系统（弹出菜单），应用需调用 [`rgpui::init_all`](crate::init_all)
//! 或 [`rgpui::menu::init`](crate::menu::init) 完成初始化。
//!
//! ## 按键提示（KeyTips）
//!
//! 调用 [`rgpui::init_all`](crate::init_all) 或 [`init`] 后，在功能区获得焦点时按下
//! `Alt` / `F10` 会进入按键提示模式：先显示各选项卡的提示键，选定选项卡后显示其中
//! 命令的提示键，命中即派发命令。提示键可经 [`CommandDescriptor::with_keytip`]
//! (crate::CommandDescriptor::with_keytip) 统一登记，也可在选项卡/按钮上逐个覆盖，
//! 未指定时退化为从标签首个 ASCII 字母派生。
//!
//! ```ignore
//! use rgpui::ribbon::{Ribbon, RibbonGroup, RibbonItem, RibbonState, RibbonTab};
//!
//! let state = cx.new(|cx| RibbonState::new(cx));
//! cx.observe(&state, |_, _, cx| cx.notify()).detach();
//!
//! Ribbon::new(&state, vec![
//!     RibbonTab::new("home", "开始")
//!         .keytip("H")
//!         .group(RibbonGroup::new("剪贴板").item(RibbonItem::button("app::Paste"))),
//! ])
//! ```

use crate::keytips::{
    KeyTip, KeyTipMatch, KeyTipScope, derive_keytip, keytip_visible, resolve_key,
};
use crate::util::FluentBuilder as _;
use crate::{
    ActiveTheme as _, App, DropdownMenu as _, Entity, FocusHandle, Hsla, Icon, IconName,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement, RenderOnce, SharedString,
    Sizable as _, StatefulInteractiveElement as _, Styled, Window, div, h_flex, px, v_flex,
};

actions!(ribbon, [ToggleKeyTips]);

/// 初始化功能区的按键提示快捷键（`Alt` / `F10`）。
///
/// 已包含在 [`rgpui::init_all`](crate::init_all) 中；单独使用功能区时可自行调用。
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("alt", ToggleKeyTips, Some("Ribbon")),
        KeyBinding::new("f10", ToggleKeyTips, Some("Ribbon")),
    ]);
}

/// 功能区状态：当前激活的选项卡索引与按键提示状态。
pub struct RibbonState {
    active_tab: usize,
    keytip_scope: KeyTipScope,
    keytip_pending: String,
    focus_handle: FocusHandle,
}

impl RibbonState {
    /// 创建状态（默认选中第 0 个选项卡、按键提示未激活）。
    pub fn new(cx: &mut crate::Context<Self>) -> Self {
        Self {
            active_tab: 0,
            keytip_scope: KeyTipScope::Hidden,
            keytip_pending: String::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// 当前激活的选项卡索引。
    pub fn active_tab(&self) -> usize {
        self.active_tab
    }

    /// 切换激活选项卡并通知订阅者。
    pub fn set_active_tab(&mut self, index: usize, cx: &mut crate::Context<Self>) {
        if self.active_tab != index {
            self.active_tab = index;
            cx.notify();
        }
    }

    /// 功能区根节点的焦点句柄（按键提示需要焦点才能接收按键）。
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// 当前按键提示层级。
    pub fn keytip_scope(&self) -> KeyTipScope {
        self.keytip_scope
    }

    /// 已输入但尚未定位到唯一候选的按键前缀（用于显示）。
    pub fn keytip_pending(&self) -> &str {
        &self.keytip_pending
    }

    /// 进入按键提示模式（显示选项卡提示）并聚焦功能区根节点。
    pub fn show_keytips(&mut self, window: &mut Window, cx: &mut crate::Context<Self>) {
        self.keytip_scope = KeyTipScope::Tabs;
        self.keytip_pending.clear();
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// 退出按键提示模式。
    pub fn hide_keytips(&mut self, cx: &mut crate::Context<Self>) {
        if self.keytip_scope != KeyTipScope::Hidden {
            self.keytip_scope = KeyTipScope::Hidden;
            self.keytip_pending.clear();
            cx.notify();
        }
    }

    /// 切换按键提示模式。
    pub fn toggle_keytips(&mut self, window: &mut Window, cx: &mut crate::Context<Self>) {
        if self.keytip_scope == KeyTipScope::Hidden {
            self.show_keytips(window, cx);
        } else {
            self.hide_keytips(cx);
        }
    }

    /// 在按键提示模式下处理一次按键。
    ///
    /// `tab_keytips` 按选项卡顺序排列（空串表示该选项卡无提示），
    /// `control_commands` 是当前选项卡内可执行项的 `(提示键, 命令 id)`（按展示顺序）。
    /// 通常由 [`Ribbon`] 的根节点按键监听调用，应用无需直接调用。
    pub fn handle_key(
        &mut self,
        key: &str,
        tab_keytips: &[SharedString],
        control_commands: &[(SharedString, SharedString)],
        cx: &mut crate::Context<Self>,
    ) {
        if self.keytip_scope == KeyTipScope::Hidden {
            return;
        }
        if key == "escape" {
            self.hide_keytips(cx);
            return;
        }

        match self.keytip_scope {
            KeyTipScope::Hidden => {}
            KeyTipScope::Tabs => {
                let prefix = std::mem::take(&mut self.keytip_pending);
                match resolve_key(&prefix, key, tab_keytips) {
                    KeyTipMatch::Match(index) => {
                        self.active_tab = index;
                        self.keytip_scope = KeyTipScope::Controls;
                        self.keytip_pending.clear();
                        cx.notify();
                    }
                    KeyTipMatch::Pending => {
                        self.keytip_pending = format!("{prefix}{key}").to_ascii_uppercase();
                        cx.notify();
                    }
                    KeyTipMatch::None => {
                        self.keytip_pending.clear();
                        cx.notify();
                    }
                }
            }
            KeyTipScope::Controls => {
                let prefix = std::mem::take(&mut self.keytip_pending);
                let keys: Vec<&str> = control_commands
                    .iter()
                    .map(|(keytip, _)| keytip.as_ref())
                    .collect();

                match resolve_key(&prefix, key, &keys) {
                    KeyTipMatch::Match(index) => {
                        let command = control_commands[index].1.clone();
                        self.hide_keytips(cx);
                        if let Err(error) = cx.dispatch_command(&command, None) {
                            log::warn!("ribbon：派发命令 `{command}` 失败：{error}");
                        }
                    }
                    KeyTipMatch::Pending => {
                        self.keytip_pending = format!("{prefix}{key}").to_ascii_uppercase();
                        cx.notify();
                    }
                    KeyTipMatch::None => {
                        // 命中不到命令时，回退尝试切换到其它选项卡（功能区常见行为）。
                        if let KeyTipMatch::Match(index) = resolve_key("", key, tab_keytips) {
                            self.active_tab = index;
                        }
                        self.keytip_pending.clear();
                        cx.notify();
                    }
                }
            }
        }
    }
}

impl RibbonState {
    /// 从按键提示解析出的命令 id 列表构造控制项（按键提示 → 当前选项卡命令）。
    fn control_commands(tab: Option<&RibbonTab>) -> Vec<(SharedString, SharedString)> {
        let Some(tab) = tab else {
            return Vec::new();
        };

        let mut commands: Vec<(SharedString, SharedString)> = Vec::new();
        for group in &tab.groups {
            for item in &group.items {
                match item {
                    RibbonItem::Button(button) => {
                        if let Some(keytip) =
                            item_keytip(&button.command, &button.label, &button.keytip)
                        {
                            commands.push((keytip, button.command.clone()));
                        }
                    }
                    RibbonItem::Toggle(toggle) => {
                        if let Some(keytip) =
                            item_keytip(&toggle.command, &toggle.label, &toggle.keytip)
                        {
                            commands.push((keytip, toggle.command.clone()));
                        }
                    }
                    // 下拉按钮的提示键激活其默认（首项）命令，与分裂按钮的行为一致。
                    RibbonItem::Dropdown(dropdown) => {
                        if let (Some(keytip), Some(default)) =
                            (dropdown_keytip(dropdown), dropdown.items.first())
                        {
                            commands.push((keytip, default.command.clone()));
                        }
                    }
                    RibbonItem::Separator => {}
                }
            }
        }
        commands
    }
}

/// 上下文选项卡的视觉上下文（如「图片工具 · 格式」）。
pub struct RibbonContext {
    label: SharedString,
    color: Hsla,
}

impl RibbonContext {
    /// 创建上下文（`label` 为工具名，`color` 为强调色）。
    pub fn new(label: impl Into<SharedString>, color: impl Into<Hsla>) -> Self {
        Self {
            label: label.into(),
            color: color.into(),
        }
    }
}

/// 一个功能区选项卡。
pub struct RibbonTab {
    id: SharedString,
    label: SharedString,
    groups: Vec<RibbonGroup>,
    context: Option<RibbonContext>,
    keytip: Option<SharedString>,
}

impl RibbonTab {
    /// 创建选项卡（`id` 需全局唯一，用于元素 id）。
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            groups: Vec::new(),
            context: None,
            keytip: None,
        }
    }

    /// 追加一个分组。
    pub fn group(mut self, group: RibbonGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// 标记为上下文选项卡（带强调色）。
    pub fn context(mut self, context: RibbonContext) -> Self {
        self.context = Some(context);
        self
    }

    /// 设置按键提示（KeyTips）。
    pub fn keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }
}

/// 选项卡内的一个分组，底部显示分组名。
pub struct RibbonGroup {
    label: SharedString,
    items: Vec<RibbonItem>,
}

impl RibbonGroup {
    /// 创建分组。
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// 追加一个命令项。
    pub fn item(mut self, item: impl Into<RibbonItem>) -> Self {
        self.items.push(item.into());
        self
    }
}

/// 功能区中的一个命令项。
pub enum RibbonItem {
    /// 普通按钮描述符（大/小由 `large` 决定）。
    Button(RibbonButton),
    /// 切换按钮。
    Toggle(RibbonToggle),
    /// 下拉按钮。
    Dropdown(RibbonDropdown),
    /// 分组内分隔符。
    Separator,
}

impl RibbonItem {
    /// 创建普通按钮（默认小按钮）。
    pub fn button(command: impl Into<SharedString>) -> RibbonButton {
        RibbonButton::new(command)
    }

    /// 创建切换按钮。
    pub fn toggle(command: impl Into<SharedString>) -> RibbonToggle {
        RibbonToggle::new(command)
    }

    /// 创建下拉按钮。
    pub fn dropdown(label: impl Into<SharedString>) -> RibbonDropdown {
        RibbonDropdown::new(label)
    }

    /// 创建分隔符。
    pub fn separator() -> Self {
        RibbonItem::Separator
    }
}

/// 普通按钮。
pub struct RibbonButton {
    command: SharedString,
    label: Option<SharedString>,
    icon: Option<IconName>,
    large: bool,
    keytip: Option<SharedString>,
}

impl RibbonButton {
    /// 创建按钮，绑定命令 id。
    pub fn new(command: impl Into<SharedString>) -> Self {
        Self {
            command: command.into(),
            label: None,
            icon: None,
            large: false,
            keytip: None,
        }
    }

    /// 覆盖显示标签（默认取注册表标签）。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 渲染为大按钮。
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// 覆盖按键提示（默认取注册表提示或从标签派生）。
    pub fn keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }
}

impl From<RibbonButton> for RibbonItem {
    fn from(value: RibbonButton) -> Self {
        RibbonItem::Button(value)
    }
}

/// 切换按钮（`checked` 状态由调用方提供）。
pub struct RibbonToggle {
    command: SharedString,
    label: Option<SharedString>,
    icon: Option<IconName>,
    checked: bool,
    keytip: Option<SharedString>,
}

impl RibbonToggle {
    /// 创建切换按钮，绑定命令 id。
    pub fn new(command: impl Into<SharedString>) -> Self {
        Self {
            command: command.into(),
            label: None,
            icon: None,
            checked: false,
            keytip: None,
        }
    }

    /// 覆盖显示标签。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 设置图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 设置选中状态。
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// 覆盖按键提示（默认取注册表提示或从标签派生）。
    pub fn keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }
}

impl From<RibbonToggle> for RibbonItem {
    fn from(value: RibbonToggle) -> Self {
        RibbonItem::Toggle(value)
    }
}

/// 下拉按钮的一项。
pub struct RibbonDropdownItem {
    command: SharedString,
    label: Option<SharedString>,
}

impl RibbonDropdownItem {
    /// 创建下拉项，绑定命令 id。
    pub fn new(command: impl Into<SharedString>) -> Self {
        Self {
            command: command.into(),
            label: None,
        }
    }

    /// 覆盖显示标签。
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// 下拉按钮。
pub struct RibbonDropdown {
    label: SharedString,
    icon: Option<IconName>,
    items: Vec<RibbonDropdownItem>,
    large: bool,
    keytip: Option<SharedString>,
}

impl RibbonDropdown {
    /// 创建下拉按钮。
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            items: Vec::new(),
            large: false,
            keytip: None,
        }
    }

    /// 设置图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 渲染为大按钮。
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// 追加一个下拉项。
    pub fn item(mut self, item: RibbonDropdownItem) -> Self {
        self.items.push(item);
        self
    }

    /// 覆盖按键提示（默认从标签派生，命中时执行首个下拉项）。
    pub fn keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }
}

impl From<RibbonDropdown> for RibbonItem {
    fn from(value: RibbonDropdown) -> Self {
        RibbonItem::Dropdown(value)
    }
}

/// 功能区元素。
#[derive(IntoElement)]
pub struct Ribbon {
    state: Entity<RibbonState>,
    tabs: Vec<RibbonTab>,
    collapsed: bool,
}

impl Ribbon {
    /// 创建功能区（`state` 建议由父视图 `observe`，切换选项卡时重绘）。
    pub fn new(state: &Entity<RibbonState>, tabs: Vec<RibbonTab>) -> Self {
        Self {
            state: state.clone(),
            tabs,
            collapsed: false,
        }
    }

    /// 设置折叠态（仅显示选项卡条）。
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl RenderOnce for Ribbon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (active_tab, scope, pending, focus_handle) = {
            let state = self.state.read(cx);
            (
                state.active_tab(),
                state.keytip_scope(),
                state.keytip_pending().to_string(),
                state.focus_handle().clone(),
            )
        };
        let show_tab_keytips = scope == KeyTipScope::Tabs;
        let show_control_keytips = scope == KeyTipScope::Controls;

        let tabs = self.tabs;
        let state = self.state.clone();

        let tab_keytips: Vec<SharedString> = tabs.iter().map(tab_keytip).collect();
        let control_commands = RibbonState::control_commands(tabs.get(active_tab));

        let tab_strip = h_flex()
            .w_full()
            .items_center()
            .gap(px(2.0))
            .px(px(8.0))
            .pt(px(4.0))
            .border_b_1()
            .border_color(theme.border)
            .children(tabs.iter().enumerate().map(|(index, tab)| {
                render_tab(
                    tab,
                    index,
                    active_tab,
                    &state,
                    show_tab_keytips,
                    &pending,
                    cx,
                )
            }));

        let mut ribbon = v_flex().w_full().bg(theme.background).child(tab_strip);

        if !self.collapsed {
            if let Some(tab) = tabs.get(active_tab) {
                let body = h_flex()
                    .w_full()
                    .items_stretch()
                    .px(px(6.0))
                    .py(px(4.0))
                    .children(tab.groups.iter().enumerate().map(|(index, group)| {
                        let is_last = index + 1 == tab.groups.len();
                        render_group(group, is_last, show_control_keytips, &pending, cx)
                    }));
                ribbon = ribbon.child(body);
            }
        }

        let state_for_action = state.clone();
        let state_for_keys = state;
        let tab_keytips_for_keys = tab_keytips;
        let control_commands_for_keys = control_commands;

        ribbon
            .track_focus(&focus_handle)
            .key_context("Ribbon")
            .on_action(move |_: &ToggleKeyTips, window, cx| {
                state_for_action.update(cx, |state, cx| state.toggle_keytips(window, cx));
            })
            .on_key_down(move |event, _window, cx| {
                let key = event.keystroke.key.to_ascii_lowercase();
                state_for_keys.update(cx, |state, cx| {
                    state.handle_key(&key, &tab_keytips_for_keys, &control_commands_for_keys, cx);
                });
            })
    }
}

/// 渲染一个选项卡按钮。
fn render_tab(
    tab: &RibbonTab,
    index: usize,
    active_tab: usize,
    state: &Entity<RibbonState>,
    show_keytips: bool,
    pending: &str,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let is_active = index == active_tab;
    let state = state.clone();
    let accent = tab
        .context
        .as_ref()
        .map(|context| context.color)
        .unwrap_or(theme.accent_foreground);
    let label = tab
        .context
        .as_ref()
        .map(|context| format!("{} · {}", context.label, tab.label))
        .unwrap_or_else(|| tab.label.to_string());
    let badge = keytip_badge(
        &tab_keytip(tab),
        pending,
        show_keytips,
        tab.context.as_ref().map(|c| c.color),
    );

    div()
        .id(SharedString::from(format!("ribbon-tab-{}", tab.id)))
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(10.0))
        .py(px(5.0))
        .rounded_md()
        .text_sm()
        .cursor_pointer()
        .when(is_active, |this| {
            this.bg(theme.background)
                .text_color(theme.foreground)
                .border_b_1()
                .border_color(accent)
        })
        .when(!is_active, |this| {
            this.text_color(theme.muted_foreground)
                .hover(|style| style.bg(theme.muted).text_color(theme.foreground))
        })
        .child(label)
        .when_some(badge, |this, badge| this.child(badge))
        .on_click(move |_event, _window, cx| {
            state.update(cx, |state, cx| state.set_active_tab(index, cx));
        })
}

/// 渲染一个分组（内容区 + 底部标签）。
fn render_group(
    group: &RibbonGroup,
    is_last: bool,
    show_keytips: bool,
    pending: &str,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();

    let content = v_flex()
        .flex_1()
        .justify_between()
        .items_center()
        .px(px(6.0))
        .pt(px(4.0))
        .pb(px(2.0))
        .child(
            h_flex()
                .flex_1()
                .items_start()
                .gap(px(4.0))
                .children(render_group_items(&group.items, show_keytips, pending, cx)),
        )
        .child(
            div()
                .pt(px(2.0))
                .text_size(px(10.0))
                .text_color(theme.muted_foreground)
                .child(group.label.clone()),
        );

    div()
        .flex()
        .when(!is_last, |this| {
            this.border_r_1().border_color(theme.border).pr(px(4.0))
        })
        .child(content)
}

/// 将分组项目排列为横向序列，连续的普通小按钮聚成每列最多 3 个的竖列。
fn render_group_items(
    items: &[RibbonItem],
    show_keytips: bool,
    pending: &str,
    cx: &App,
) -> Vec<crate::AnyElement> {
    let mut elements: Vec<crate::AnyElement> = Vec::new();
    let mut small_column: Vec<crate::AnyElement> = Vec::new();

    let mut flush_column = |elements: &mut Vec<crate::AnyElement>,
                            small_column: &mut Vec<crate::AnyElement>| {
        if !small_column.is_empty() {
            let column = v_flex().gap(px(2.0)).children(std::mem::take(small_column));
            elements.push(column.into_any_element());
        }
    };

    for item in items {
        match item {
            RibbonItem::Button(button) if !button.large => {
                small_column.push(render_small_button(button, show_keytips, pending, cx));
                if small_column.len() >= 3 {
                    flush_column(&mut elements, &mut small_column);
                }
            }
            RibbonItem::Button(button) => {
                flush_column(&mut elements, &mut small_column);
                elements.push(
                    render_large_button(button, show_keytips, pending, cx).into_any_element(),
                );
            }
            RibbonItem::Toggle(toggle) => {
                flush_column(&mut elements, &mut small_column);
                elements.push(render_toggle(toggle, show_keytips, pending, cx).into_any_element());
            }
            RibbonItem::Dropdown(dropdown) => {
                flush_column(&mut elements, &mut small_column);
                elements
                    .push(render_dropdown(dropdown, show_keytips, pending, cx).into_any_element());
            }
            RibbonItem::Separator => {
                flush_column(&mut elements, &mut small_column);
                let theme = cx.theme();
                elements.push(
                    div()
                        .w(px(1.0))
                        .h(px(56.0))
                        .mx(px(3.0))
                        .bg(theme.border)
                        .into_any_element(),
                );
            }
        }
    }
    flush_column(&mut elements, &mut small_column);

    elements
}

/// 从注册表解析命令标签（未登记时回退为命令 id）。
fn command_label(command: &SharedString, override_label: &Option<SharedString>) -> SharedString {
    if let Some(label) = override_label {
        return label.clone();
    }
    crate::CommandRegistry::global()
        .get(command)
        .map(|descriptor| descriptor.label().clone())
        .unwrap_or_else(|| command.clone())
}

/// 派发命令的回调（吞掉错误，仅记日志）。
fn dispatch(
    command: &SharedString,
) -> impl Fn(&crate::ClickEvent, &mut Window, &mut App) + 'static {
    let command = command.clone();
    move |_event, _window, cx| {
        if let Err(error) = cx.dispatch_command(&command, None) {
            log::warn!("ribbon：派发命令 `{command}` 失败：{error}");
        }
    }
}

/// 选项卡的按键提示（显式覆盖优先，其次从标签派生）。
fn tab_keytip(tab: &RibbonTab) -> SharedString {
    tab.keytip
        .clone()
        .or_else(|| derive_keytip(&tab.label))
        .unwrap_or_default()
}

/// 命令项的按键提示：显式覆盖 > 注册表登记 > 从标签派生。
fn item_keytip(
    command: &SharedString,
    label: &Option<SharedString>,
    override_keytip: &Option<SharedString>,
) -> Option<SharedString> {
    if let Some(keytip) = override_keytip {
        return Some(keytip.clone());
    }
    if let Some(descriptor) = crate::CommandRegistry::global().get(command) {
        if let Some(keytip) = descriptor.keytip() {
            return Some(keytip.clone());
        }
    }
    derive_keytip(&command_label(command, label))
}

/// 下拉按钮的按键提示（显式覆盖优先，其次从标签派生）。
fn dropdown_keytip(dropdown: &RibbonDropdown) -> Option<SharedString> {
    dropdown
        .keytip
        .clone()
        .or_else(|| derive_keytip(&dropdown.label))
}

/// 在按键提示模式下，为匹配当前前缀的提示生成徽标。
fn keytip_badge(keytip: &str, pending: &str, active: bool, color: Option<Hsla>) -> Option<KeyTip> {
    if active && keytip_visible(keytip, pending) {
        let badge = KeyTip::new(keytip.to_string());
        Some(match color {
            Some(color) => badge.color(color),
            None => badge,
        })
    } else {
        None
    }
}

/// 渲染一个小按钮（图标 + 标签横排）。
fn render_small_button(
    button: &RibbonButton,
    show_keytips: bool,
    pending: &str,
    cx: &App,
) -> crate::AnyElement {
    let theme = cx.theme();
    let label = command_label(&button.command, &button.label);
    let foreground = theme.foreground;
    let badge = item_keytip(&button.command, &button.label, &button.keytip)
        .and_then(|keytip| keytip_badge(&keytip, pending, show_keytips, None));
    let mut element = h_flex()
        .id(button.command.clone())
        .items_center()
        .gap(px(6.0))
        .px(px(6.0))
        .py(px(3.0))
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .text_color(foreground)
        .hover(|style| style.bg(theme.muted));

    if let Some(icon) = button.icon.clone() {
        element = element.child(Icon::new(icon).with_size(px(16.0)).text_color(foreground));
    }

    element
        .child(label)
        .when_some(badge, |this, badge| this.child(badge))
        .on_click(dispatch(&button.command))
        .into_any_element()
}

/// 渲染一个大按钮（图标在上、标签在下）。
fn render_large_button(
    button: &RibbonButton,
    show_keytips: bool,
    pending: &str,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let label = command_label(&button.command, &button.label);
    let foreground = theme.foreground;
    let badge = item_keytip(&button.command, &button.label, &button.keytip)
        .and_then(|keytip| keytip_badge(&keytip, pending, show_keytips, None));
    let mut element = v_flex()
        .id(button.command.clone())
        .w(px(64.0))
        .items_center()
        .justify_start()
        .gap(px(4.0))
        .px(px(4.0))
        .py(px(6.0))
        .rounded_md()
        .cursor_pointer()
        .text_color(foreground)
        .hover(|style| style.bg(theme.muted));

    if let Some(icon) = button.icon.clone() {
        element = element.child(Icon::new(icon).with_size(px(24.0)).text_color(foreground));
    }

    element
        .child(div().text_size(px(11.0)).text_center().child(label))
        .when_some(badge, |this, badge| this.child(badge))
        .on_click(dispatch(&button.command))
}

/// 渲染一个切换按钮。
fn render_toggle(
    toggle: &RibbonToggle,
    show_keytips: bool,
    pending: &str,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let label = command_label(&toggle.command, &toggle.label);
    let foreground = theme.foreground;
    let badge = item_keytip(&toggle.command, &toggle.label, &toggle.keytip)
        .and_then(|keytip| keytip_badge(&keytip, pending, show_keytips, None));
    let mut element = v_flex()
        .id(toggle.command.clone())
        .w(px(64.0))
        .items_center()
        .gap(px(4.0))
        .px(px(4.0))
        .py(px(6.0))
        .rounded_md()
        .cursor_pointer()
        .text_color(foreground)
        .when(toggle.checked, |this| this.bg(theme.accent))
        .when(!toggle.checked, |this| {
            this.hover(|style| style.bg(theme.muted))
        });

    if let Some(icon) = toggle.icon.clone() {
        element = element.child(Icon::new(icon).with_size(px(24.0)).text_color(foreground));
    }

    element
        .child(div().text_size(px(11.0)).text_center().child(label))
        .when_some(badge, |this, badge| this.child(badge))
        .on_click(dispatch(&toggle.command))
}

/// 渲染一个下拉按钮（触发按钮 + 弹出菜单，菜单项按 id 派发）。
fn render_dropdown(
    dropdown: &RibbonDropdown,
    show_keytips: bool,
    pending: &str,
    _cx: &App,
) -> impl IntoElement {
    let label = dropdown.label.clone();
    let badge = dropdown_keytip(dropdown)
        .and_then(|keytip| keytip_badge(&keytip, pending, show_keytips, None));
    let items: Vec<(SharedString, SharedString)> = dropdown
        .items
        .iter()
        .map(|item| {
            (
                item.command.clone(),
                command_label(&item.command, &item.label),
            )
        })
        .collect();

    let mut trigger = crate::Button::new(SharedString::from(format!(
        "ribbon-dropdown-{}",
        dropdown.label
    )))
    .label(label);

    if let Some(icon) = dropdown.icon.clone() {
        trigger = trigger.icon(icon);
    }

    let trigger = trigger.dropdown_menu(move |menu, _window, _cx| {
        let mut menu = menu;
        for (command, label) in &items {
            let command = command.clone();
            menu = menu.item(crate::PopupMenuItem::new(label.clone()).on_click(
                move |_event, _window, cx| {
                    let _ = cx.dispatch_command(&command, None);
                },
            ));
        }
        menu
    });

    h_flex()
        .items_center()
        .gap(px(4.0))
        .child(trigger)
        .when_some(badge, |this, badge| this.child(badge))
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{Ribbon, RibbonGroup, RibbonItem, RibbonState, RibbonTab, command_label};
    use crate::keytips::KeyTipScope;
    use crate::{AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window};

    /// 供按键提示测试使用的、登记了提示键的命令。
    fn demo_save() -> crate::CommandDescriptor {
        crate::CommandDescriptor::new("rgpui_test::RibbonSave", "保存").with_keytip("S")
    }

    crate::register_command!(demo_save);

    /// 测试宿主视图：持有一个 [`RibbonState`] 实体并渲染功能区。
    struct Probe {
        state: Entity<RibbonState>,
    }

    impl Render for Probe {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            Ribbon::new(
                &self.state,
                vec![
                    RibbonTab::new("home", "开始").keytip("H").group(
                        RibbonGroup::new("剪贴板")
                            .item(
                                RibbonItem::button("rgpui_test::Open")
                                    .label("打开")
                                    .keytip("O"),
                            )
                            .item(RibbonItem::separator()),
                    ),
                    RibbonTab::new("insert", "插入").keytip("N").group(
                        RibbonGroup::new("插入").item(RibbonItem::button("rgpui_test::Quit")),
                    ),
                ],
            )
        }
    }

    /// 类似 [`Probe`]，但只用于按键提示状态测试。
    fn probe_tabs() -> Vec<RibbonTab> {
        vec![
            RibbonTab::new("home", "开始").keytip("H").group(
                RibbonGroup::new("剪贴板").item(
                    RibbonItem::button("rgpui_test::Open")
                        .label("打开")
                        .keytip("O"),
                ),
            ),
            RibbonTab::new("insert", "插入").keytip("N").group(
                RibbonGroup::new("插入").item(RibbonItem::button("rgpui_test::Quit").keytip("Q")),
            ),
        ]
    }

    /// 状态默认选中第 0 个选项卡。
    #[rgpui::test]
    fn state_defaults_to_first_tab(cx: &mut crate::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| RibbonState::new(cx));
            Probe { state }
        });
        let state = view.read_with(cx, |probe, _| probe.state.clone());
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 0);
    }

    /// 选项卡提示键：显式覆盖优先，否则从标签派生。
    #[test]
    fn tab_keytip_derives_from_label() {
        assert_eq!(
            super::tab_keytip(&RibbonTab::new("x", "Home")).as_ref(),
            "H"
        );
        assert_eq!(
            super::tab_keytip(&RibbonTab::new("x", "主页").keytip("Z")).as_ref(),
            "Z"
        );
    }

    /// 命令项提示键优先取注册表登记值，显式覆盖优先级最高。
    #[test]
    fn item_keytip_prefers_registry_then_override() {
        let command = SharedString::from("rgpui_test::RibbonSave");
        let from_registry = super::item_keytip(&command, &None, &None);
        assert_eq!(from_registry.as_deref(), Some("S"));

        let overridden = super::item_keytip(&command, &None, &Some(SharedString::from("X")));
        assert_eq!(overridden.as_deref(), Some("X"));
    }

    /// 显式标签优先于注册表标签。
    #[test]
    fn command_label_prefers_override() {
        let command = SharedString::from("rgpui_test::Open");
        let label = command_label(&command, &Some(SharedString::from("自定义")));
        assert_eq!(label.as_ref(), "自定义");
    }

    /// 未显式指定时从命令注册表读取标签。
    #[test]
    fn command_label_falls_back_to_registry() {
        let command = SharedString::from("rgpui_test::Open");
        let label = command_label(&command, &None);
        assert_eq!(label.as_ref(), "打开");
    }

    /// 未登记的命令回退为命令 id。
    #[test]
    fn command_label_falls_back_to_id() {
        let command = SharedString::from("rgpui_test::Missing");
        let label = command_label(&command, &None);
        assert_eq!(label, command);
    }

    /// 功能区零交互代码可绘制，切换选项卡后可重绘。
    #[rgpui::test]
    fn renders_and_switches_tabs(cx: &mut crate::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| RibbonState::new(cx));
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            Probe { state }
        });

        cx.update(|window, cx| {
            _ = window.draw(cx);
        });

        let state = view.read_with(cx, |probe, _| probe.state.clone());
        state.update(cx, |state, cx| state.set_active_tab(1, cx));
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 1);

        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    /// 按键提示：默认隐藏，`Alt`/`F10` 进入选项层，逐层按键选择选项卡并派发命令。
    #[rgpui::test]
    fn keytip_flow_selects_tab_and_dispatches(cx: &mut crate::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| RibbonState::new(cx));
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            Probe { state }
        });
        let state = view.read_with(cx, |probe, _| probe.state.clone());
        assert_eq!(
            state.read_with(cx, |state, _| state.keytip_scope()),
            KeyTipScope::Hidden
        );

        // 进入按键提示模式 -> 选项卡层。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.show_keytips(window, cx));
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.keytip_scope()),
            KeyTipScope::Tabs
        );

        let tabs = probe_tabs();
        let tab_keytips: Vec<SharedString> = tabs.iter().map(super::tab_keytip).collect();
        let controls = RibbonState::control_commands(tabs.first());

        // 按 H -> 切到「开始」并进入其命令层。
        state.update(cx, |state, cx| {
            state.handle_key("h", &tab_keytips, &controls, cx);
        });
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 0);
        assert_eq!(
            state.read_with(cx, |state, _| state.keytip_scope()),
            KeyTipScope::Controls
        );

        // 按 O -> 命中「打开」，退出按键提示模式。
        state.update(cx, |state, cx| {
            state.handle_key("o", &tab_keytips, &controls, cx);
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.keytip_scope()),
            KeyTipScope::Hidden
        );

        // 命中不到的按键保持等待，Esc 退出。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.show_keytips(window, cx));
        });
        state.update(cx, |state, cx| {
            state.handle_key("escape", &tab_keytips, &controls, cx);
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.keytip_scope()),
            KeyTipScope::Hidden
        );
    }
}
