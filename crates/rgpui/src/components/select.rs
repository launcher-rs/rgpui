//! 下拉选择框（表单版）。
//!
//! 比裸 `DropdownMenu` 更顺手的值列表选择：选项 + 选中索引 + 占位符 +
//! 变更回调一次配齐。触发器为普通按钮，下拉列表为 `PopupMenu`。

use crate::{prelude::FluentBuilder as _, *};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Select 实例计数器，用于生成默认唯一 ID（同页多实例不冲突）。
static SELECT_ID: AtomicU64 = AtomicU64::new(0);

/// 下拉选择框。
#[derive(IntoElement)]
pub struct Select {
    /// 元素 ID（默认唯一生成）。
    id: SharedString,
    /// 选项列表。
    options: Vec<SharedString>,
    /// 选中索引。
    selected: Option<usize>,
    /// 无选中时的占位文本。
    placeholder: SharedString,
    /// 变更回调（索引, 值）。
    on_change:
        Option<Arc<dyn Fn(usize, &SharedString, &mut Window, &mut App) + Send + Sync + 'static>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl Select {
    /// 创建下拉选择框。
    pub fn new(options: Vec<SharedString>) -> Self {
        let id = SELECT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id: SharedString::from(format!("select-{id}")),
            options,
            selected: None,
            placeholder: "请选择".into(),
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置元素 ID（默认唯一生成，多实例一般不用管）。
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    /// 设置选中索引。
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// 设置变更回调。
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: Fn(usize, &SharedString, &mut Window, &mut App) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(f));
        self
    }
}

impl Styled for Select {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let current = self
            .selected
            .and_then(|ix| self.options.get(ix).cloned())
            .unwrap_or_else(|| self.placeholder.clone());
        let options = self.options;
        let on_change = self.on_change;
        let user_style = self.style;

        Button::new(self.id.clone())
            .label(current)
            .icon(IconName::ChevronDown)
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
            .dropdown_menu(move |menu, _, _| {
                options
                    .iter()
                    .enumerate()
                    .fold(menu, |menu, (ix, option)| {
                        let option = option.clone();
                        let on_change = on_change.clone();
                        menu.item(PopupMenuItem::label(option.clone()).on_click(
                            move |_, window, cx| {
                                if let Some(ref cb) = on_change {
                                    cb(ix, &option, window, cx);
                                }
                            },
                        ))
                    })
            })
    }
}
