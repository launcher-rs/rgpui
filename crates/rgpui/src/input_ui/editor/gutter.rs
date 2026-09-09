//! 自定义 gutter 列（`editor` feature 门控）：行号左侧第三列，用户可注册。
//!
//! 对标 VS Code 行号沟槽：run 按钮 / 调试断点 / 书签等都落在这里。
//! 模型：具名 provider（`id` + 开关 + 构造闭包），`EditorState` 方法增删开关；
//! 构造闭包逐可见行调用（`Fn(buffer_row) -> Option<AnyElement>`，无标记返回
//! `None`），元素自带点击行为；总开关默认关（用户自主开启）。
//!
//! 渲染：`layout_gutter_markers`（prepaint，fold 图标同款：固定格预排）+
//! `paint_gutter_markers`（paint）；列宽 [`GUTTER_WIDTH`]，开时行号区加宽，
//! 文本/折叠图标经 `line_number_width` 自动右移，行号绘制 x 显式偏移（见 element）。
//! provider 闭包要求轻量（哈希查表级；逐行逐帧调用，文档注明）。

use std::rc::Rc;

use crate::{AnyElement, App, Entity, Pixels, Point, SharedString, Window, point, px, size};

use super::super::layout::LastLayout;
use super::state::EditorState;

/// gutter 列宽（标记格固定尺寸，行高见布局）。
pub(crate) const GUTTER_WIDTH: Pixels = crate::px(20.);

/// gutter 标记构造器（buffer 行 → 标记元素；无标记返回 `None`）。
///
/// 闭包须轻量（逐可见行逐帧调用）；需文本/状态时自行捕获实体读取。
pub type GutterMarkerBuilder = Rc<dyn Fn(usize, &mut Window, &mut App) -> Option<AnyElement>>;

/// 单个 gutter provider（具名 + 独立开关）。
#[derive(Clone)]
pub(crate) struct GutterMarkerEntry {
    /// provider 标识（增删开关用）。
    pub(super) id: SharedString,
    /// 是否启用。
    pub(super) enabled: bool,
    /// 标记构造器。
    pub(super) build: GutterMarkerBuilder,
}

/// gutter 标记布局（prepaint 产物；paint 阶段逐个绘制）。
#[derive(Default)]
pub(crate) struct GutterMarkersLayout {
    /// 列宽（0 = 关闭，无标记时 paint 跳过）。
    pub(crate) width: Pixels,
    /// 预排好的标记元素。
    items: Vec<AnyElement>,
}

impl EditorState {
    /// 注册 gutter provider（同 id 覆盖；默认启用）。
    pub fn add_gutter_provider(
        &self,
        id: impl Into<SharedString>,
        builder: impl Fn(usize, &mut Window, &mut App) -> Option<AnyElement> + 'static,
        cx: &mut App,
    ) {
        let entry = GutterMarkerEntry {
            id: id.into(),
            enabled: true,
            build: Rc::new(builder),
        };
        let _ = self.input.update(cx, |state, cx| {
            if let Some(slot) = state.gutter_markers.iter_mut().find(|e| e.id == entry.id) {
                *slot = entry;
            } else {
                state.gutter_markers.push(entry);
            }
            cx.notify();
        });
    }

    /// 移除 gutter provider（无此 id 不做事）。
    pub fn remove_gutter_provider(&self, id: &str, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            let before = state.gutter_markers.len();
            state.gutter_markers.retain(|e| e.id.as_ref() != id);
            if state.gutter_markers.len() != before {
                cx.notify();
            }
        });
    }

    /// 单个 provider 开关。
    pub fn set_gutter_provider_enabled(&self, id: &str, enabled: bool, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            if let Some(slot) = state
                .gutter_markers
                .iter_mut()
                .find(|e| e.id.as_ref() == id)
            {
                slot.enabled = enabled;
                cx.notify();
            }
        });
    }

    /// gutter 列总开关（默认关；用户自主开启）。
    pub fn set_gutter_column_enabled(&self, enabled: bool, cx: &mut App) {
        let _ = self.input.update(cx, |state, cx| {
            state.gutter_column_enabled = enabled;
            cx.notify();
        });
    }

    /// gutter 列是否开启。
    pub fn gutter_column_enabled(&self, cx: &App) -> bool {
        self.input
            .read_with(cx, |state, _| state.gutter_column_enabled)
    }

    /// provider 列表（id + 开关；设置页回显用）。
    pub fn gutter_providers(&self, cx: &App) -> Vec<(SharedString, bool)> {
        self.input.read_with(cx, |state, _| {
            state
                .gutter_markers
                .iter()
                .map(|e| (e.id.clone(), e.enabled))
                .collect()
        })
    }
}

/// gutter 列宽（开且有启用的 provider 才占宽，否则 0；行号区加宽用）。
pub(crate) fn gutter_column_width(state: &super::super::InputState) -> Pixels {
    if state.gutter_column_enabled && state.gutter_markers.iter().any(|e| e.enabled) {
        GUTTER_WIDTH
    } else {
        px(0.)
    }
}

/// 布局 gutter 标记（prepaint；行号区最左固定格，fold 图标同款预排）。
pub(crate) fn layout_gutter_markers(
    state: &Entity<super::super::InputState>,
    origin: Point<Pixels>,
    last_layout: &LastLayout,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> GutterMarkersLayout {
    let builders = state.read_with(cx, |state, _| {
        state
            .gutter_markers
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.build.clone())
            .collect::<Vec<GutterMarkerBuilder>>()
    });
    if builders.is_empty() {
        return GutterMarkersLayout::default();
    }
    let mut items = Vec::new();
    // 与文本绘制同循环（行高累加口径一致，fold 图标同款）；标记取行顶 y。
    let mut y = last_layout.visible_top;
    for (line_layout, &buffer_row) in last_layout
        .lines
        .iter()
        .zip(last_layout.visible_buffer_lines.iter())
    {
        for build in &builders {
            if let Some(mut element) = build(buffer_row, window, cx) {
                let cell_origin = point(origin.x, origin.y + y);
                element.prepaint_as_root(
                    cell_origin,
                    size(GUTTER_WIDTH, line_height).into(),
                    window,
                    cx,
                );
                items.push(element);
            }
        }
        y += line_layout.size(line_height).height;
    }
    GutterMarkersLayout {
        width: GUTTER_WIDTH,
        items,
    }
}

/// 绘制 gutter 标记（paint；预排元素逐个绘制）。
pub(crate) fn paint_gutter_markers(
    layout: &mut GutterMarkersLayout,
    window: &mut Window,
    cx: &mut App,
) {
    if layout.width == px(0.) {
        return;
    }
    for element in layout.items.iter_mut() {
        element.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppContext as _, Context, Entity, Render};

    /// 持有编辑器状态的测试宿主视图。
    struct Probe {
        state: Entity<EditorState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    /// 内部 provider 表长度（断言用）。
    fn provider_count(editor: &Entity<EditorState>, cx: &mut crate::VisualTestContext) -> usize {
        editor.read_with(cx, |state, cx| {
            state
                .input()
                .read_with(cx, |input, _| input.gutter_markers.len())
        })
    }

    /// 注册/开关/移除走通（状态层，无需渲染）。
    #[rgpui::test]
    fn provider_registry_roundtrip(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx, "fn main() {}\n"));
            Probe { state: editor }
        });
        let editor = probe.read_with(cx, |probe, _| probe.state.clone());
        // 默认关闭。
        assert!(!editor.read_with(cx, |state, cx| state.gutter_column_enabled(cx)));
        // 注册两个 provider（同 id 覆盖，只留一个）。
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.add_gutter_provider("run", |_, _, _| None, cx);
                state.add_gutter_provider("bp", |_, _, _| None, cx);
                state.add_gutter_provider("run", |_, _, _| None, cx);
            });
        });
        assert_eq!(provider_count(&editor, cx), 2);
        // 单个开关。
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_gutter_provider_enabled("bp", false, cx);
            });
        });
        let enabled: Vec<bool> = editor.read_with(cx, |state, cx| {
            state.input().read_with(cx, |input, _| {
                input.gutter_markers.iter().map(|e| e.enabled).collect()
            })
        });
        assert!(enabled.contains(&true) && enabled.contains(&false));
        // 总开关 + 移除。
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_gutter_column_enabled(true, cx);
            });
        });
        assert!(editor.read_with(cx, |state, cx| state.gutter_column_enabled(cx)));
        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.remove_gutter_provider("bp", cx);
                state.remove_gutter_provider("nonexistent", cx);
            });
        });
        assert_eq!(provider_count(&editor, cx), 1);
    }
}
