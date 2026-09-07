//! Dock 布局：IDE 式可停靠面板系统。
//!
//! 左/右/底部/中央四个区域，每个区域是一组标签页；标签页可点击切换、关闭，
//! 可拖拽到其他区域（`Draggable` + `DropZone`），布局可序列化为 JSON 持久化。
//! 面板内容由调用方经工厂闭包提供（每次渲染调用），框架只管布局与标签状态。

use super::{DragData, Draggable, DropZone, DropZoneStyle};
use crate::{prelude::FluentBuilder as _, *};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Dock 区域位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockPosition {
    /// 左侧。
    Left,
    /// 右侧。
    Right,
    /// 底部。
    Bottom,
    /// 中央（常驻，不可隐藏）。
    Center,
}

impl DockPosition {
    /// JSON/调试用短名。
    fn name(&self) -> &'static str {
        match self {
            DockPosition::Left => "left",
            DockPosition::Right => "right",
            DockPosition::Bottom => "bottom",
            DockPosition::Center => "center",
        }
    }

    /// 由短名解析（未知返回 `None`）。
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "left" => Some(DockPosition::Left),
            "right" => Some(DockPosition::Right),
            "bottom" => Some(DockPosition::Bottom),
            "center" => Some(DockPosition::Center),
            _ => None,
        }
    }

    /// 全部区域（固定顺序：左、中、右，底部独立一行）。
    fn all() -> [DockPosition; 4] {
        [
            DockPosition::Left,
            DockPosition::Center,
            DockPosition::Right,
            DockPosition::Bottom,
        ]
    }
}

/// Dock 面板（内容工厂 + 元信息，状态实体持有）。
pub struct DockPanel {
    /// 面板唯一 ID（跨区域拖拽与持久化的键）。
    pub id: SharedString,
    /// 标签页标题。
    pub title: SharedString,
    /// 所属区域。
    pub position: DockPosition,
    /// 是否可关闭。
    pub closable: bool,
    /// 内容工厂（每次渲染调用）。
    pub content: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
}

impl DockPanel {
    /// 创建面板。
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        position: DockPosition,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            position,
            closable: true,
            content: Rc::new(content),
        }
    }

    /// 设置是否可关闭。
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// Dock 区域状态实体（父组件 `cx.new(|_| DockAreaState::new())` 持有）。
pub struct DockAreaState {
    panels: Vec<DockPanel>,
    active: HashMap<DockPosition, SharedString>,
    hidden: HashSet<DockPosition>,
    left_width: Pixels,
    right_width: Pixels,
    bottom_height: Pixels,
}

impl DockAreaState {
    /// 创建空 Dock 状态（默认左 240、右 280、底部 200）。
    pub fn new() -> Self {
        Self {
            panels: Vec::new(),
            active: HashMap::new(),
            hidden: HashSet::new(),
            left_width: px(240.0),
            right_width: px(280.0),
            bottom_height: px(200.0),
        }
    }

    /// 添加面板（同 ID 已存在则替换元信息并激活）。
    pub fn add_panel(&mut self, panel: DockPanel, cx: &mut Context<Self>) {
        let id = panel.id.clone();
        let position = panel.position;
        if let Some(existing) = self.panels.iter_mut().find(|p| p.id == id) {
            existing.title = panel.title;
            existing.position = position;
            existing.closable = panel.closable;
            existing.content = panel.content;
        } else {
            self.panels.push(panel);
        }
        self.active.insert(position, id);
        self.hidden.remove(&position);
        cx.notify();
    }

    /// 移除面板（返回是否删过）。
    pub fn remove_panel(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(ix) = self.panels.iter().position(|p| p.id == id) else {
            return false;
        };
        let position = self.panels[ix].position;
        self.panels.remove(ix);
        // 激活项若被删，退到同区域第一个。
        if self
            .active
            .get(&position)
            .is_some_and(|active| active == id)
        {
            self.active.remove(&position);
            if let Some(next) = self.panels.iter().find(|p| p.position == position) {
                self.active.insert(position, next.id.clone());
            }
        }
        cx.notify();
        true
    }

    /// 移动面板到指定区域并激活。
    pub fn move_panel(&mut self, id: &str, position: DockPosition, cx: &mut Context<Self>) {
        if let Some(panel) = self.panels.iter_mut().find(|p| p.id == id) {
            panel.position = position;
            self.active.insert(position, panel.id.clone());
            self.hidden.remove(&position);
            cx.notify();
        }
    }

    /// 激活某区域的指定面板。
    pub fn set_active(&mut self, position: DockPosition, id: &str, cx: &mut Context<Self>) {
        if self
            .panels
            .iter()
            .any(|p| p.id == id && p.position == position)
        {
            self.active.insert(position, id.into());
            cx.notify();
        }
    }

    /// 显示/隐藏区域（中央不可隐藏）。
    pub fn set_visible(&mut self, position: DockPosition, visible: bool, cx: &mut Context<Self>) {
        if position == DockPosition::Center {
            return;
        }
        if visible {
            self.hidden.remove(&position);
        } else {
            self.hidden.insert(position);
        }
        cx.notify();
    }

    /// 区域是否可见（无面板时也不可见）。
    pub fn is_visible(&self, position: DockPosition) -> bool {
        position == DockPosition::Center
            || (!self.hidden.contains(&position)
                && self.panels.iter().any(|p| p.position == position))
    }

    /// 设置左侧宽度。
    pub fn set_left_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.left_width = width;
        cx.notify();
    }

    /// 设置右侧宽度。
    pub fn set_right_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.right_width = width;
        cx.notify();
    }

    /// 设置底部高度。
    pub fn set_bottom_height(&mut self, height: Pixels, cx: &mut Context<Self>) {
        self.bottom_height = height;
        cx.notify();
    }

    /// 某区域的面板（注册顺序）。
    fn region_panels(&self, position: DockPosition) -> Vec<&DockPanel> {
        self.panels
            .iter()
            .filter(|p| p.position == position)
            .collect()
    }

    /// 某区域当前激活面板 ID（无则退首个）。
    fn region_active(&self, position: DockPosition) -> Option<SharedString> {
        if let Some(active) = self.active.get(&position) {
            if self.panels.iter().any(|p| p.id == *active) {
                return Some(active.clone());
            }
        }
        self.panels
            .iter()
            .find(|p| p.position == position)
            .map(|p| p.id.clone())
    }

    /// 导出布局 JSON（面板位置/激活项/区域尺寸；内容工厂不序列化，恢复时调用方重新注册）。
    pub fn layout_json(&self) -> String {
        let panels: Vec<serde_json::Value> = self
            .panels
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id.to_string(),
                    "title": p.title.to_string(),
                    "position": p.position.name(),
                    "closable": p.closable,
                })
            })
            .collect();
        let active: HashMap<&str, String> = self
            .active
            .iter()
            .map(|(pos, id)| (pos.name(), id.to_string()))
            .collect();
        serde_json::json!({
            "version": 1,
            "panels": panels,
            "active": active,
            "left_width": self.left_width.0,
            "right_width": self.right_width.0,
            "bottom_height": self.bottom_height.0,
        })
        .to_string()
    }

    /// 由布局 JSON 恢复位置/激活/尺寸（面板内容需调用方先 `add_panel` 注册，占位条目会被替换）。
    pub fn restore_layout(&mut self, json: &str, cx: &mut Context<Self>) -> bool {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
            return false;
        };
        let Some(panels) = value.get("panels").and_then(|v| v.as_array()) else {
            return false;
        };
        for item in panels {
            let (Some(id), Some(position)) = (
                item.get("id").and_then(|v| v.as_str()),
                item.get("position")
                    .and_then(|v| v.as_str())
                    .and_then(DockPosition::from_name),
            ) else {
                continue;
            };
            if let Some(panel) = self.panels.iter_mut().find(|p| p.id == id) {
                panel.position = position;
            }
        }
        self.active.clear();
        if let Some(active) = value.get("active").and_then(|v| v.as_object()) {
            for (pos_name, id) in active {
                if let (Some(position), Some(id)) = (DockPosition::from_name(pos_name), id.as_str())
                {
                    self.active.insert(position, id.into());
                }
            }
        }
        if let Some(w) = value.get("left_width").and_then(|v| v.as_f64()) {
            self.left_width = px(w as f32);
        }
        if let Some(w) = value.get("right_width").and_then(|v| v.as_f64()) {
            self.right_width = px(w as f32);
        }
        if let Some(h) = value.get("bottom_height").and_then(|v| v.as_f64()) {
            self.bottom_height = px(h as f32);
        }
        cx.notify();
        true
    }
}

impl Default for DockAreaState {
    fn default() -> Self {
        Self::new()
    }
}

/// Dock 区域元素（状态实体 + 布局渲染）。
#[derive(IntoElement)]
pub struct DockArea {
    state: Entity<DockAreaState>,
    style: StyleRefinement,
}

impl DockArea {
    /// 由状态实体创建。
    pub fn new(state: Entity<DockAreaState>) -> Self {
        Self {
            state,
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for DockArea {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DockArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let muted_foreground = theme.tokens.muted_foreground.color;
        let accent = theme.tokens.accent.color;
        let panel_bg = theme.tokens.popover;
        let border = theme.tokens.border.color;
        let user_style = self.style;
        let state = self.state.clone();

        // 快照：各区域面板与激活项（内容在各区域内按需调用工厂）。
        let snapshot = self.state.read(cx);
        let mut region_ids: HashMap<DockPosition, Vec<(SharedString, SharedString, bool)>> =
            HashMap::new();
        for position in DockPosition::all() {
            region_ids.insert(
                position,
                snapshot
                    .region_panels(position)
                    .iter()
                    .map(|p| (p.id.clone(), p.title.clone(), p.closable))
                    .collect(),
            );
        }
        let active: HashMap<DockPosition, Option<SharedString>> = DockPosition::all()
            .into_iter()
            .map(|pos| (pos, snapshot.region_active(pos)))
            .collect();
        let (left_width, right_width, bottom_height) = (
            snapshot.left_width,
            snapshot.right_width,
            snapshot.bottom_height,
        );
        let visible: HashMap<DockPosition, bool> = DockPosition::all()
            .into_iter()
            .map(|pos| (pos, snapshot.is_visible(pos)))
            .collect();

        // 单个区域（含标签头 + 内容 + 拖放）。
        let mut render_region = |position: DockPosition, size: Box<dyn FnOnce(Div) -> Div>| {
            let panels = region_ids.get(&position).cloned().unwrap_or_default();
            if panels.is_empty() || !visible.get(&position).copied().unwrap_or(false) {
                return div().into_any_element();
            }
            let active_id = active.get(&position).cloned().flatten();

            // 标签头。
            let mut header = div().flex().flex_row().items_center().gap(px(2.0));
            for (id, title, closable) in &panels {
                let is_active = active_id.as_ref() == Some(id);
                let tab_state = state.clone();
                let tab_id = id.clone();
                let tab_title = title.clone();
                let mut tab = div()
                    .id(format!("dock-tab-body-{id}"))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_active, |this| this.bg(accent.opacity(0.12)))
                    .when(!is_active, |this| {
                        this.hover(|this| this.bg(accent.opacity(0.06)))
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(if is_active { accent } else { muted_foreground })
                            .child(tab_title.clone()),
                    )
                    .on_click(move |_, _, cx| {
                        tab_state.update(cx, |state, cx| {
                            state.set_active(position, &tab_id, cx);
                        });
                    });
                if *closable {
                    let close_state = state.clone();
                    let close_id = id.clone();
                    tab = tab.child(
                        div()
                            .id(format!("dock-close-{id}"))
                            .text_xs()
                            .text_color(muted_foreground)
                            .cursor_pointer()
                            .child("×")
                            .on_click(move |_, _, cx| {
                                close_state.update(cx, |state, cx| {
                                    state.remove_panel(&close_id, cx);
                                });
                            }),
                    );
                }
                // 标签页可拖拽到其他区域。
                header = header.child(
                    Draggable::new(
                        format!("dock-tab-{id}"),
                        DragData::new(id.clone()).with_label(title.clone()),
                    )
                    .child(tab),
                );
            }

            // 激活面板内容（调用工厂）。
            let content = self
                .state
                .read_with(cx, |state, _| {
                    active_id.as_ref().and_then(|active| {
                        state
                            .panels
                            .iter()
                            .find(|p| &p.id == active)
                            .map(|p| p.content.clone())
                    })
                })
                .map(|factory| factory(window, cx));
            let mut body = div().flex_1().overflow_hidden();
            if let Some(content) = content {
                body = body.child(content);
            }

            let region = div()
                .flex()
                .flex_col()
                .h_full()
                .bg(panel_bg)
                .child(header)
                .child(div().h(px(1.0)).w_full().bg(border))
                .child(body);
            let region = size(region);
            // 整个区域是放置目标：其他区域的标签可拖过来。
            let drop_state = state.clone();
            DropZone::<SharedString>::new(format!("dock-drop-{}", position.name()))
                .drop_zone_style(DropZoneStyle::Filled)
                .on_drop(move |data: &DragData<SharedString>, _, cx| {
                    drop_state.update(cx, |state, cx| {
                        state.move_panel(&data.data, position, cx);
                    });
                })
                .child(region)
                .into_any_element()
        };

        let left = render_region(DockPosition::Left, Box::new(move |d| d.w(left_width)));
        let center = render_region(DockPosition::Center, Box::new(move |d| d.flex_1()));
        let right = render_region(DockPosition::Right, Box::new(move |d| d.w(right_width)));
        let bottom = render_region(
            DockPosition::Bottom,
            Box::new(move |d| d.w_full().h(bottom_height)),
        );

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(left)
                    .child(center)
                    .child(right),
            )
            .child(bottom)
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
