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
//!  → [`RibbonItem`]：大按钮 / 小按钮 / 切换按钮 / 下拉按钮 / 分裂按钮 / 库 / 组合框 / 分隔符。
//!
//! 选项卡条右侧带折叠开关（展开态朝上、折叠态朝下）；折叠态下点击选项卡会自动展开
//! 并切换。
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

/// 功能区状态：当前激活的选项卡索引、折叠态与按键提示状态。
pub struct RibbonState {
    active_tab: usize,
    collapsed: bool,
    keytip_scope: KeyTipScope,
    keytip_pending: String,
    focus_handle: FocusHandle,
}

impl RibbonState {
    /// 创建状态（默认选中第 0 个选项卡、展开、按键提示未激活）。
    pub fn new(cx: &mut crate::Context<Self>) -> Self {
        Self {
            active_tab: 0,
            collapsed: false,
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

    /// 点击选项卡：切换激活项，并在折叠态下顺带展开功能区（Office 行为）。
    pub fn activate_tab(&mut self, index: usize, cx: &mut crate::Context<Self>) {
        if self.active_tab == index && !self.collapsed {
            return;
        }
        self.active_tab = index;
        self.collapsed = false;
        cx.notify();
    }

    /// 功能区是否处于折叠态（仅显示选项卡条）。
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// 设置折叠态并通知订阅者。
    pub fn set_collapsed(&mut self, collapsed: bool, cx: &mut crate::Context<Self>) {
        if self.collapsed != collapsed {
            self.collapsed = collapsed;
            cx.notify();
        }
    }

    /// 在展开 / 折叠之间切换。
    pub fn toggle_collapsed(&mut self, cx: &mut crate::Context<Self>) {
        self.set_collapsed(!self.collapsed, cx);
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
                    // 分裂按钮的提示键激活主命令。
                    RibbonItem::Split(split) => {
                        if let Some(keytip) =
                            item_keytip(&split.command, &split.label, &split.keytip)
                        {
                            commands.push((keytip, split.command.clone()));
                        }
                    }
                    // 库的提示键激活首个库项。
                    RibbonItem::Gallery(gallery) => {
                        if let (Some(keytip), Some(first)) =
                            (gallery_keytip(gallery), gallery.items.first())
                        {
                            commands.push((keytip, first.command.clone()));
                        }
                    }
                    // 组合框的提示键激活当前选中项（无选中时取首项）。
                    RibbonItem::ComboBox(combo) => {
                        if let Some(keytip) = combo_keytip(combo) {
                            let selected = combo
                                .selected
                                .and_then(|ix| combo.items.get(ix))
                                .or_else(|| combo.items.first());
                            if let Some(item) = selected {
                                commands.push((keytip, item.command.clone()));
                            }
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
    /// 分裂按钮（主命令 + 下拉附加项）。
    Split(RibbonSplitButton),
    /// 库（弹出网格供选择）。
    Gallery(RibbonGallery),
    /// 组合框（当前值 + 下拉选项列表）。
    ComboBox(RibbonComboBox),
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

    /// 创建分裂按钮（绑定主命令）。
    pub fn split(command: impl Into<SharedString>) -> RibbonSplitButton {
        RibbonSplitButton::new(command)
    }

    /// 创建库（`label` 为触发按钮文案）。
    pub fn gallery(label: impl Into<SharedString>) -> RibbonGallery {
        RibbonGallery::new(label)
    }

    /// 创建组合框（`id` 为稳定元素 id）。
    pub fn combobox(id: impl Into<SharedString>) -> RibbonComboBox {
        RibbonComboBox::new(id)
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

/// 分裂按钮：主区域派发主命令，右侧（或下方）下拉箭头展开附加项。
///
/// 与 Office 的「分裂按钮 / 拆分按钮」一致：直接点击走常用命令，点箭头取更多
/// 变体；两者共用同一个命令 id 绑定与派发链路。
pub struct RibbonSplitButton {
    command: SharedString,
    label: Option<SharedString>,
    icon: Option<IconName>,
    large: bool,
    keytip: Option<SharedString>,
    items: Vec<RibbonDropdownItem>,
}

impl RibbonSplitButton {
    /// 创建分裂按钮，绑定主命令 id。
    pub fn new(command: impl Into<SharedString>) -> Self {
        Self {
            command: command.into(),
            label: None,
            icon: None,
            large: false,
            keytip: None,
            items: Vec::new(),
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

    /// 追加一个下拉附加项。
    pub fn item(mut self, item: RibbonDropdownItem) -> Self {
        self.items.push(item);
        self
    }

    /// 覆盖按键提示（默认取注册表提示或从标签派生，命中时执行主命令）。
    pub fn keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }
}

impl From<RibbonSplitButton> for RibbonItem {
    fn from(value: RibbonSplitButton) -> Self {
        RibbonItem::Split(value)
    }
}

/// 库中的一项（图标 + 标签，可标记选中）。
pub struct RibbonGalleryItem {
    command: SharedString,
    label: Option<SharedString>,
    icon: Option<IconName>,
    selected: bool,
}

impl RibbonGalleryItem {
    /// 创建库项，绑定命令 id。
    pub fn new(command: impl Into<SharedString>) -> Self {
        Self {
            command: command.into(),
            label: None,
            icon: None,
            selected: false,
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

    /// 标记为当前选中项（绘制强调边框）。
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

/// 库：触发按钮展开一个图标网格，点击任一项派发其命令。
///
/// 对应 Office 的「库 / Gallery」（如样式库、形状库）。网格以弹出菜单承载，
/// 点击单元格后自动关闭弹出层。
pub struct RibbonGallery {
    label: SharedString,
    icon: Option<IconName>,
    items: Vec<RibbonGalleryItem>,
    columns: usize,
    large: bool,
    keytip: Option<SharedString>,
}

impl RibbonGallery {
    /// 创建库（`label` 为触发按钮文案）。
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            items: Vec::new(),
            columns: 4,
            large: false,
            keytip: None,
        }
    }

    /// 设置触发按钮图标。
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 追加一个库项。
    pub fn item(mut self, item: RibbonGalleryItem) -> Self {
        self.items.push(item);
        self
    }

    /// 设置网格列数（默认 4）。
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// 渲染为大按钮（默认小按钮）。
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// 覆盖按键提示（默认从标签派生，命中时执行首个库项）。
    pub fn keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }
}

impl From<RibbonGallery> for RibbonItem {
    fn from(value: RibbonGallery) -> Self {
        RibbonItem::Gallery(value)
    }
}

/// 组合框中的一项。
pub struct RibbonComboBoxItem {
    command: SharedString,
    label: Option<SharedString>,
}

impl RibbonComboBoxItem {
    /// 创建组合框项，绑定命令 id。
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

/// 组合框：展示当前值，下拉选择后派发对应命令。
///
/// 对应 Office 的字号 / 样式组合框。当前为「只读下拉」形态：每一项绑定命令 id，
/// 选中即派发，显示文本取选中项标签（或占位符）。
pub struct RibbonComboBox {
    id: SharedString,
    placeholder: SharedString,
    selected: Option<usize>,
    items: Vec<RibbonComboBoxItem>,
    width: Option<f32>,
    large: bool,
    keytip: Option<SharedString>,
}

impl RibbonComboBox {
    /// 创建组合框（`id` 需在当前功能区范围内唯一）。
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            placeholder: SharedString::from("请选择"),
            selected: None,
            items: Vec::new(),
            width: None,
            large: false,
            keytip: None,
        }
    }

    /// 追加一个选项。
    pub fn item(mut self, item: RibbonComboBoxItem) -> Self {
        self.items.push(item);
        self
    }

    /// 设置选中项索引。
    pub fn selected(mut self, index: usize) -> Self {
        self.selected = Some(index);
        self
    }

    /// 设置无选中时的占位文本。
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// 设置固定宽度（像素）。
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// 渲染为大按钮（默认小按钮）。
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// 覆盖按键提示（默认从选中项标签派生，命中时执行选中项命令）。
    pub fn keytip(mut self, keytip: impl Into<SharedString>) -> Self {
        self.keytip = Some(keytip.into());
        self
    }
}

impl From<RibbonComboBox> for RibbonItem {
    fn from(value: RibbonComboBox) -> Self {
        RibbonItem::ComboBox(value)
    }
}

/// 功能区元素。
#[derive(IntoElement)]
pub struct Ribbon {
    state: Entity<RibbonState>,
    tabs: Vec<RibbonTab>,
}

impl Ribbon {
    /// 创建功能区（`state` 建议由父视图 `observe`，切换选项卡/折叠时重绘）。
    pub fn new(state: &Entity<RibbonState>, tabs: Vec<RibbonTab>) -> Self {
        Self {
            state: state.clone(),
            tabs,
        }
    }
}

impl RenderOnce for Ribbon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (active_tab, collapsed, scope, pending, focus_handle) = {
            let state = self.state.read(cx);
            (
                state.active_tab(),
                state.is_collapsed(),
                state.keytip_scope(),
                state.keytip_pending().to_string(),
                state.focus_handle().clone(),
            )
        };
        let show_tab_keytips = scope == KeyTipScope::Tabs;
        let show_control_keytips = scope == KeyTipScope::Controls;

        let tabs = self.tabs;
        let state = self.state;

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
            }))
            // 选项卡条右侧的折叠开关（展开时朝上、折叠时朝下）。
            .child(render_collapse_toggle(&state, collapsed, cx));

        let mut ribbon = v_flex().w_full().bg(theme.background).child(tab_strip);

        if !collapsed {
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
            state.update(cx, |state, cx| state.activate_tab(index, cx));
        })
}

/// 渲染选项卡条右侧的折叠开关：展开态显示朝上箭头（点击折叠），
/// 折叠态显示朝下箭头（点击展开）。
fn render_collapse_toggle(
    state: &Entity<RibbonState>,
    collapsed: bool,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let state = state.clone();
    let icon = if collapsed {
        IconName::ChevronDown
    } else {
        IconName::ChevronUp
    };
    h_flex()
        .ml_auto()
        .id("ribbon-collapse-toggle")
        .items_center()
        .justify_center()
        .size(px(22.0))
        .rounded_md()
        .cursor_pointer()
        .text_color(theme.muted_foreground)
        .hover(|style| style.bg(theme.muted).text_color(theme.foreground))
        .child(Icon::new(icon).with_size(px(14.0)))
        .on_click(move |_event, _window, cx| {
            state.update(cx, |state, cx| state.toggle_collapsed(cx));
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
            RibbonItem::Split(split) => {
                flush_column(&mut elements, &mut small_column);
                elements.push(render_split(split, show_keytips, pending, cx).into_any_element());
            }
            RibbonItem::Gallery(gallery) => {
                flush_column(&mut elements, &mut small_column);
                elements
                    .push(render_gallery(gallery, show_keytips, pending, cx).into_any_element());
            }
            RibbonItem::ComboBox(combo) => {
                flush_column(&mut elements, &mut small_column);
                elements.push(render_combo(combo, show_keytips, pending, cx).into_any_element());
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

/// 库的按键提示（显式覆盖优先，其次从触发按钮标签派生）。
fn gallery_keytip(gallery: &RibbonGallery) -> Option<SharedString> {
    gallery
        .keytip
        .clone()
        .or_else(|| derive_keytip(&gallery.label))
}

/// 组合框的按键提示：显式覆盖 > 当前项注册表提示 > 从当前项标签派生。
fn combo_keytip(combo: &RibbonComboBox) -> Option<SharedString> {
    if let Some(keytip) = &combo.keytip {
        return Some(keytip.clone());
    }
    let current = combo
        .selected
        .and_then(|ix| combo.items.get(ix))
        .or_else(|| combo.items.first())?;
    if let Some(descriptor) = crate::CommandRegistry::global().get(&current.command) {
        if let Some(keytip) = descriptor.keytip() {
            return Some(keytip.clone());
        }
    }
    derive_keytip(&command_label(&current.command, &current.label))
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

/// 渲染一个分裂按钮：主区域派发主命令 + 右侧下拉箭头展开附加项。
fn render_split(
    split: &RibbonSplitButton,
    show_keytips: bool,
    pending: &str,
    cx: &App,
) -> impl IntoElement {
    // 主区域直接复用普通按钮的渲染（含大/小与按键提示徽标）。
    let main_button = RibbonButton {
        command: split.command.clone(),
        label: split.label.clone(),
        icon: split.icon.clone(),
        large: split.large,
        keytip: split.keytip.clone(),
    };
    let main = if split.large {
        render_large_button(&main_button, show_keytips, pending, cx).into_any_element()
    } else {
        render_small_button(&main_button, show_keytips, pending, cx)
    };

    let items: Vec<(SharedString, SharedString)> = split
        .items
        .iter()
        .map(|item| {
            (
                item.command.clone(),
                command_label(&item.command, &item.label),
            )
        })
        .collect();

    let caret = crate::Button::new(SharedString::from(format!(
        "ribbon-split-{}-caret",
        split.command
    )))
    .compact()
    .icon(IconName::ChevronDown)
    .dropdown_menu(move |menu, _window, _cx| {
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
        .gap(px(1.0))
        .child(main)
        .child(caret)
}

/// 渲染一个库：触发按钮展开图标网格，点击单元格派发其命令。
fn render_gallery(
    gallery: &RibbonGallery,
    show_keytips: bool,
    pending: &str,
    _cx: &App,
) -> impl IntoElement {
    let label = gallery.label.clone();
    let badge = gallery_keytip(gallery)
        .and_then(|keytip| keytip_badge(&keytip, pending, show_keytips, None));
    let columns = gallery.columns;
    let items: std::rc::Rc<Vec<GalleryEntry>> = std::rc::Rc::new(
        gallery
            .items
            .iter()
            .map(|item| GalleryEntry {
                command: item.command.clone(),
                label: command_label(&item.command, &item.label),
                icon: item.icon.clone(),
                selected: item.selected,
            })
            .collect(),
    );

    let mut trigger = crate::Button::new(SharedString::from(format!(
        "ribbon-gallery-{}",
        gallery.label
    )))
    .label(label);

    if let Some(icon) = gallery.icon.clone() {
        trigger = trigger.icon(icon);
    }

    let trigger = trigger.dropdown_menu(move |menu, _window, _cx| {
        let items = items.clone();
        menu.max_w(px(320.0))
            .item(crate::PopupMenuItem::element(move |_window, cx| {
                gallery_grid(&items, columns, cx)
            }))
    });

    h_flex()
        .items_center()
        .gap(px(4.0))
        .child(trigger)
        .when_some(badge, |this, badge| this.child(badge))
}

/// 库网格中的一项（渲染用的展开数据）。
struct GalleryEntry {
    command: SharedString,
    label: SharedString,
    icon: Option<IconName>,
    selected: bool,
}

/// 构建库的图标网格（点击单元格派发对应命令，并冒泡到弹出项以关闭弹层）。
fn gallery_grid(items: &[GalleryEntry], columns: usize, cx: &App) -> crate::AnyElement {
    let theme = cx.theme();
    let mut grid = v_flex().gap(px(4.0));

    for chunk in items.chunks(columns.max(1)) {
        let mut row = h_flex().gap(px(4.0));
        for entry in chunk {
            let command = entry.command.clone();
            let mut cell = v_flex()
                .id(entry.command.clone())
                .w(px(72.0))
                .items_center()
                .gap(px(2.0))
                .p(px(4.0))
                .rounded_md()
                .cursor_pointer()
                .when(entry.selected, |this| {
                    this.border_1().border_color(theme.accent)
                })
                .hover(|style| style.bg(theme.muted));
            if let Some(icon) = entry.icon.clone() {
                cell = cell.child(Icon::new(icon).with_size(px(20.0)));
            }
            cell = cell.child(
                div()
                    .text_size(px(10.0))
                    .text_center()
                    .child(entry.label.clone()),
            );
            row = row.child(cell.on_click(move |_event, _window, cx| {
                let _ = cx.dispatch_command(&command, None);
            }));
        }
        grid = grid.child(row);
    }

    grid.into_any_element()
}

/// 渲染一个组合框：展示当前值，下拉选择后派发对应命令。
fn render_combo(
    combo: &RibbonComboBox,
    show_keytips: bool,
    pending: &str,
    _cx: &App,
) -> impl IntoElement {
    let current = combo
        .selected
        .and_then(|ix| combo.items.get(ix))
        .map(|item| command_label(&item.command, &item.label))
        .unwrap_or_else(|| combo.placeholder.clone());
    let badge =
        combo_keytip(combo).and_then(|keytip| keytip_badge(&keytip, pending, show_keytips, None));
    let selected = combo.selected;
    let items: Vec<(SharedString, SharedString)> = combo
        .items
        .iter()
        .map(|item| {
            (
                item.command.clone(),
                command_label(&item.command, &item.label),
            )
        })
        .collect();

    let mut trigger = crate::Button::new(SharedString::from(format!("ribbon-combo-{}", combo.id)))
        .label(current)
        .icon(IconName::ChevronsUpDown);

    if let Some(width) = combo.width {
        trigger = trigger.w(px(width));
    }

    let trigger = trigger.dropdown_menu(move |menu, _window, _cx| {
        let mut menu = menu;
        for (index, (command, label)) in items.iter().enumerate() {
            let command = command.clone();
            menu = menu.item(
                crate::PopupMenuItem::new(label.clone())
                    .checked(selected == Some(index))
                    .on_click(move |_event, _window, cx| {
                        let _ = cx.dispatch_command(&command, None);
                    }),
            );
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
    use super::{
        Ribbon, RibbonComboBox, RibbonComboBoxItem, RibbonGallery, RibbonGalleryItem, RibbonGroup,
        RibbonItem, RibbonSplitButton, RibbonState, RibbonTab, command_label,
    };
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
                            .item(
                                RibbonItem::split("rgpui_test::RibbonSave")
                                    .label("保存")
                                    .keytip("S")
                                    .item(super::RibbonDropdownItem::new("rgpui_test::Quit")),
                            )
                            .item(
                                RibbonItem::gallery("样式")
                                    .keytip("Y")
                                    .columns(2)
                                    .item(RibbonGalleryItem::new("rgpui_test::Open").selected(true))
                                    .item(RibbonGalleryItem::new("rgpui_test::Quit")),
                            )
                            .item(
                                RibbonItem::combobox("size")
                                    .selected(0)
                                    .placeholder("字号")
                                    .item(RibbonComboBoxItem::new("rgpui_test::Open")),
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

    /// 折叠态默认关闭；切换后生效；折叠态下点击选项卡会展开并切换。
    #[rgpui::test]
    fn collapse_toggles_and_tab_click_expands(cx: &mut crate::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_window, cx| {
            let state = cx.new(|cx| RibbonState::new(cx));
            cx.observe(&state, |_, _, cx| cx.notify()).detach();
            Probe { state }
        });
        let state = view.read_with(cx, |probe, _| probe.state.clone());

        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));
        state.update(cx, |state, cx| state.toggle_collapsed(cx));
        assert!(state.read_with(cx, |state, _| state.is_collapsed()));

        // 折叠态下点击（激活）第二个选项卡：切换并展开。
        state.update(cx, |state, cx| state.activate_tab(1, cx));
        assert_eq!(state.read_with(cx, |state, _| state.active_tab()), 1);
        assert!(!state.read_with(cx, |state, _| state.is_collapsed()));

        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    /// 分裂 / 库 / 组合框均参与按键提示导航，命中其默认命令。
    #[test]
    fn control_commands_include_split_gallery_combo() {
        let tabs = [RibbonTab::new("home", "开始").group(
            RibbonGroup::new("组")
                .item(
                    RibbonItem::split("rgpui_test::RibbonSave")
                        .label("保存")
                        .keytip("S")
                        .item(super::RibbonDropdownItem::new("rgpui_test::Quit")),
                )
                .item(
                    RibbonItem::gallery("样式")
                        .keytip("Y")
                        .item(RibbonGalleryItem::new("rgpui_test::Open")),
                )
                .item(
                    RibbonItem::combobox("size")
                        .keytip("Z")
                        .selected(1)
                        .item(RibbonComboBoxItem::new("rgpui_test::Open"))
                        .item(RibbonComboBoxItem::new("rgpui_test::Quit")),
                ),
        )];
        let controls = RibbonState::control_commands(tabs.first());

        // 分裂按钮命中主命令、库命中首项、组合框命中当前选中项。
        assert!(controls.contains(&(
            SharedString::from("S"),
            SharedString::from("rgpui_test::RibbonSave")
        )));
        assert!(controls.contains(&(
            SharedString::from("Y"),
            SharedString::from("rgpui_test::Open")
        )));
        assert!(controls.contains(&(
            SharedString::from("Z"),
            SharedString::from("rgpui_test::Quit")
        )));
    }

    /// 库 / 组合框的提示键：显式覆盖优先，其次从标签派生。
    #[test]
    fn gallery_and_combo_keytip_fallback() {
        let gallery = RibbonGallery::new("Styles").item(RibbonGalleryItem::new("rgpui_test::Open"));
        assert_eq!(super::gallery_keytip(&gallery).as_deref(), Some("S"));

        // 组合框优先取当前项注册表提示（demo_save 登记了 "S"）。
        let combo = RibbonComboBox::new("size")
            .selected(0)
            .item(RibbonComboBoxItem::new("rgpui_test::RibbonSave"));
        assert_eq!(super::combo_keytip(&combo).as_deref(), Some("S"));

        // 显式覆盖优先级最高。
        let combo = combo.keytip("K");
        assert_eq!(super::combo_keytip(&combo).as_deref(), Some("K"));
    }

    /// 分裂按钮由主按钮与下拉箭头组成，且携带附加项。
    #[test]
    fn split_button_builders() {
        let split = RibbonSplitButton::new("rgpui_test::RibbonSave")
            .label("保存")
            .large()
            .item(super::RibbonDropdownItem::new("rgpui_test::Quit"));
        assert!(split.large);
        assert_eq!(split.items.len(), 1);
        let item: RibbonItem = split.into();
        assert!(matches!(item, RibbonItem::Split(_)));
    }
}
