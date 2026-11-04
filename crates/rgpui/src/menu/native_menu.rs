//! OS 原生弹出菜单的菜单项描述。
//!
//! 与框架自绘的 [`PopupMenu`](crate::menu::PopupMenu) 相对：这里只是**描述**一份菜单，
//! 交给平台层用系统自己的菜单控件画出来（Windows `HMENU` + `TrackPopupMenuEx`、
//! macOS `NSMenu`）。因此只保留系统菜单普遍能表达的东西 —— 标签、勾选态、禁用态、
//! 分隔线、子菜单；图标、链接、危险色这类框架自绘才有的表现力不在其中。
//!
//! 每个可点击项都带一个 `id`：菜单项被选中时平台只把 `id` 送回来，动作的含义由调用方
//! 解释（与 [`App::on_tray_menu_action`](crate::App::on_tray_menu_action) 同一口径），
//! 这样子菜单嵌套多久都不需要在平台层维护索引。

use crate::SharedString;

/// 原生菜单里的一项。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeMenuItem {
    /// 可点击的菜单项。
    Action {
        /// 显示的文本
        label: SharedString,
        /// 选中后经通道回传的标识
        id: SharedString,
        /// 置灰（不可点击）
        disabled: bool,
        /// 显示勾选标记
        checked: bool,
    },
    /// 分隔线
    Separator,
    /// 子菜单：悬停展开，内部继续嵌套 [`NativeMenuItem`]
    Submenu {
        /// 子菜单标题
        label: SharedString,
        /// 子菜单项
        items: Vec<NativeMenuItem>,
    },
}

/// 一份待弹出的原生菜单。
#[derive(Clone, Default)]
pub struct NativeMenu {
    items: Vec<NativeMenuItem>,
}

impl NativeMenu {
    /// 创建空菜单。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一个可点击项。
    pub fn action(mut self, label: impl Into<SharedString>, id: impl Into<SharedString>) -> Self {
        self.items.push(NativeMenuItem::Action {
            label: label.into(),
            id: id.into(),
            disabled: false,
            checked: false,
        });
        self
    }

    /// 追加一个带勾选标记的可点击项（常用于「开关」类设置）。
    pub fn checked(
        mut self,
        label: impl Into<SharedString>,
        id: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        self.items.push(NativeMenuItem::Action {
            label: label.into(),
            id: id.into(),
            disabled: false,
            checked,
        });
        self
    }

    /// 追加一个置灰项：仍然显示，但点不动。
    pub fn disabled(
        mut self,
        label: impl Into<SharedString>,
        id: impl Into<SharedString>,
        disabled: bool,
    ) -> Self {
        self.items.push(NativeMenuItem::Action {
            label: label.into(),
            id: id.into(),
            disabled,
            checked: false,
        });
        self
    }

    /// 追加分隔线。
    pub fn separator(mut self) -> Self {
        self.items.push(NativeMenuItem::Separator);
        self
    }

    /// 追加子菜单，`build` 在闭包里继续往 [`NativeMenu`] 上接项。
    pub fn submenu(
        mut self,
        label: impl Into<SharedString>,
        build: impl FnOnce(NativeMenu) -> NativeMenu,
    ) -> Self {
        let submenu = build(NativeMenu::new());
        self.items.push(NativeMenuItem::Submenu {
            label: label.into(),
            items: submenu.items,
        });
        self
    }

    /// 菜单项列表（平台层据此建原生菜单）。
    pub fn items(&self) -> &[NativeMenuItem] {
        &self.items
    }

    /// 没有一项时返回 `true`；调用方不该弹空菜单，平台层也据此直接拒绝。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 深度优先收集所有可点击项的 id（含子菜单），用于断言嵌套与顺序。
    fn ids_of(items: &[NativeMenuItem]) -> Vec<String> {
        let mut out = Vec::new();
        for item in items {
            match item {
                NativeMenuItem::Action { id, .. } => out.push(id.to_string()),
                NativeMenuItem::Separator => {}
                NativeMenuItem::Submenu { items, .. } => out.extend(ids_of(items)),
            }
        }
        out
    }

    /// 构建顺序、子菜单嵌套与 id 收集都要与写法一致。
    #[test]
    fn builder_keeps_order_and_nested_ids() {
        let menu = NativeMenu::new()
            .action("复制", "copy")
            .separator()
            .checked("自动换行", "wrap", true)
            .disabled("删除", "delete", true)
            .submenu("打开", |menu| {
                menu.action("新窗口", "open-new").action("默认程序", "open")
            });

        assert_eq!(menu.items().len(), 5);
        assert!(!menu.is_empty());

        assert_eq!(
            ids_of(menu.items()),
            vec!["copy", "wrap", "delete", "open-new", "open"],
            "子菜单里的项也要被收集，且按深度优先顺序"
        );

        // 勾选态与禁用态各自落到对应项上。
        let checked = menu
            .items()
            .iter()
            .filter(|item| matches!(item, NativeMenuItem::Action { checked: true, .. }))
            .count();
        let disabled = menu
            .items()
            .iter()
            .filter(|item| matches!(item, NativeMenuItem::Action { disabled: true, .. }))
            .count();
        assert_eq!(checked, 1);
        assert_eq!(disabled, 1);
    }

    /// 空菜单要能被识别出来（调用方据此不弹）。
    #[test]
    fn empty_menu_reports_empty() {
        assert!(NativeMenu::new().is_empty());
        assert!(NativeMenu::new().items().is_empty());
    }
}
