//! 检查器默认面板（I3 开箱即用）。
//!
//! `App::enable_default_inspector` 一行启用本面板（含 `Div` 布局展示注册）；
//! 应用入口再调 `window.toggle_inspector(cx)` 即出完整面板，共两行。
//! 自定义时整板替换（见 `custom` recipe：复用 [`default_inspector_panel`] 包裹横幅）
//! 或按状态类型扩展（`register_inspector_element`，默认已注册 Div 展示）。

use crate::{
    AnyElement, App, Button, ButtonVariants, ClipboardItem, Context, Div, DivInspectorState,
    Global, Inspector, InspectorElementId, IntoElement, ParentElement, SharedString, Window, div,
    h_flex, prelude::*, px, rgb, v_flex,
};
use std::collections::HashSet;
use std::sync::Arc;

/// 顶栏插槽：接管标题/状态徽标/拾取按钮/提示整块（默认见 [`default_inspector_header`]）。
pub type InspectorHeaderSlot =
    Arc<dyn Fn(&mut Inspector, &mut Window, &mut Context<Inspector>) -> AnyElement + Send + Sync>;

/// 段落插槽：包装面板自有信息卡（标题 + 正文；默认见 [`default_inspector_section`]）。
///
/// 注册表状态展示（`register_inspector_element` 接入的，如 Div 布局）不经过它，
/// 保持全自定义。
pub type InspectorSectionSlot =
    Arc<dyn Fn(Option<SharedString>, AnyElement) -> AnyElement + Send + Sync>;

/// 默认面板插槽（H5）：不整板替换也能换顶栏/段落外皮。
///
/// 经 `App::set_inspector_panel_slots` 设置（`Global`，默认空即默认外皮）；
/// `default_inspector_panel` 渲染时读取。
#[derive(Default)]
pub struct InspectorPanelSlots {
    /// 顶栏插槽（`None` 走 [`default_inspector_header`]）。
    pub header: Option<InspectorHeaderSlot>,
    /// 段落插槽（`None` 走 [`default_inspector_section`]）。
    pub section: Option<InspectorSectionSlot>,
}

impl Global for InspectorPanelSlots {}

/// 完整树单帧最多渲染行数（超量截断并提示，面板外层已可滚动）。
const FULL_TREE_ROW_CAP: usize = 800;

/// 完整树逐节点展开状态（存 App 全局）。
///
/// 只增不重置：选中变化时把新选中祖先链并入已展开集，不塌掉用户的浏览进度；
/// 收起靠单节点折叠或“全部收起”。
#[derive(Default)]
struct FullTreeUiState {
    /// 上次播种时的选中键，变化即增量播种。
    selected_key: String,
    /// 已展开节点的稳定键集合。
    expanded: HashSet<String>,
}

impl Global for FullTreeUiState {}

/// 默认检查器面板：状态徽标、拾取按钮、选中源码位置、完整树与已注册状态展示。
///
/// 经 `App::enable_default_inspector` 注册；自定义面板可直接复用本函数包裹扩展。
pub fn default_inspector_panel(
    inspector: &mut Inspector,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    // 先物化选中信息为 owned 数据，结束借用后再渲染各状态。
    let selected: Option<(String, usize)> = inspector.active_element_id().map(|id| {
        let loc = id.path.source_location;
        (format!("{}:{}", loc.file(), loc.line()), id.instance_id)
    });
    // 完整树展开集：选中变化时把新选中祖先链并入已展开集（只增不重置），
    // 点完整树只会展开更多，不会塌掉浏览进度。
    let active_id: Option<InspectorElementId> = inspector.active_element_id().cloned();
    let active_key = active_id
        .as_ref()
        .map(|id| id.tree_key())
        .unwrap_or_default();
    let tree_expanded = cx.update_default_global::<FullTreeUiState, _>(|state, _| {
        if state.selected_key != active_key {
            state.selected_key = active_key.clone();
            if let Some(id) = active_id.as_ref() {
                state.expanded.extend(ancestor_keys(window, id));
            }
        }
        state.expanded.clone()
    });
    let (tree_rows, tree_total) = snapshot_full_tree(
        window,
        active_id.as_ref(),
        &tree_expanded,
        FULL_TREE_ROW_CAP,
    );
    let states = inspector.render_inspector_states(window, cx);
    // 插槽：未设置走默认外皮（整板替换仍可用，只是多数定制到插槽即可）。
    let slots = cx.try_global::<InspectorPanelSlots>();
    let header: InspectorHeaderSlot = slots
        .and_then(|s| s.header.clone())
        .unwrap_or_else(|| Arc::new(default_inspector_header));
    let section: InspectorSectionSlot = slots
        .and_then(|s| s.section.clone())
        .unwrap_or_else(|| Arc::new(default_inspector_section));

    div()
        .id("inspector-panel")
        .debug_selector(|| "inspector-panel".to_string())
        .size_full()
        .relative()
        .bg(rgb(0xf7f7f7))
        .child(
            v_flex()
                .size_full()
                // 顶栏固定（标题/状态/拾取按钮/提示）：内容再长也不滚走，拾取随时可点。
                .child(header(inspector, window, cx))
                // 内容区独立滚动：选中卡片/完整树/状态展示再长也不顶走顶栏。
                .child(
                    div()
                        .id("inspector-panel-scroll")
                        .debug_selector(|| "inspector-scroll".to_string())
                        .flex_1()
                        .overflow_scroll()
                        .child(
                            v_flex()
                                .debug_selector(|| "inspector-content".to_string())
                                .gap(px(8.0))
                                .p(px(12.0))
                                .pt(px(4.0))
                                .when_some(selected, |this, (loc, instance)| {
                                    this.child(section(
                                        Some("选中元素".into()),
                                        v_flex()
                                            .gap(px(2.0))
                                            .child(info_row(cx, "源码", loc))
                                            .child(info_row(cx, "实例", format!("#{instance}")))
                                            .into_any_element(),
                                    ))
                                })
                                .child(full_tree_card(cx, tree_rows, tree_total, &section))
                                .child(runtime_card(window, cx, &section))
                                .child(error_card(cx, &section))
                                .children(states),
                        ),
                ),
        )
        // 左缘拖拽条（绝对定位浮层，不参与布局，避免破坏已验证的 flex 结构）：
        // 拖动调面板宽（240px ~ 视口-240px），松开/移出结束。放最后以保证命中在最上。
        .child(resize_strip())
        .into_any_element()
}

/// 面板宽拖拽状态（存 App 全局；松开/移出即结束，不会卡死）。
#[derive(Default)]
pub(crate) struct InspectorResizeState {
    pub(crate) dragging: bool,
    pub(crate) start_x: f32,
    pub(crate) start_width: f32,
}

impl Global for InspectorResizeState {}

/// 拖拽条是否正在拖动（拾取事件分发时让路用：拖动中鼠标移到画布上也不走拾取）。
pub(crate) fn is_inspector_resizing(cx: &App) -> bool {
    cx.try_global::<InspectorResizeState>()
        .is_some_and(|state| state.dragging)
}

/// 左缘拖拽条（绝对定位浮层：宽 8px、全高，悬停高亮，左右拖动调面板宽）。
fn resize_strip() -> impl IntoElement {
    div()
        .id("inspector-resize-strip")
        .debug_selector(|| "inspector-resize-strip".to_string())
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(px(8.0))
        .cursor_col_resize()
        .hover(|this| this.bg(rgb(0xcccccc)))
        .on_mouse_down(crate::MouseButton::Left, move |event, window, cx| {
            cx.stop_propagation();
            window.prevent_default();
            // 指针捕获：按下点 topmost 的 hitbox 即本条（面板最后绘制），
            // 此后移出条外移动/松开仍路由到本条监听器，拖拽不中断；
            // 松开时框架自动释放捕获。
            let pressed = window.rendered_frame.hit_test(event.position);
            if let Some(hitbox_id) = pressed.ids.first() {
                window.capture_pointer(*hitbox_id);
            }
            let start_width = window
                .inspector_width()
                .map(|w| w.as_f32())
                .unwrap_or_else(|| crate::rems(30.0).to_pixels(window.rem_size()).as_f32());
            let start_x = event.position.x.as_f32();
            cx.update_default_global::<InspectorResizeState, _>(|state, _| {
                state.dragging = true;
                state.start_x = start_x;
                state.start_width = start_width;
            });
        })
        .on_mouse_move(move |event, window, cx| {
            let dragging = cx.update_default_global::<InspectorResizeState, _>(|state, _| {
                (state.dragging, state.start_x, state.start_width)
            });
            if dragging.0 {
                // 面板在右侧：往左拖（x 变小）加宽。
                let width = dragging.2 + (dragging.1 - event.position.x.as_f32());
                let max = (window.viewport_size.width.as_f32() - 240.0).max(280.0);
                window.set_inspector_width(Some(px(width.clamp(240.0, max))));
            }
        })
        .on_mouse_up(crate::MouseButton::Left, move |_, _, cx| {
            cx.update_default_global::<InspectorResizeState, _>(|state, _| {
                state.dragging = false;
            });
        })
        .on_mouse_up_out(crate::MouseButton::Left, move |_, _, cx| {
            // 条外松开同样结束拖拽，避免卡死。
            cx.update_default_global::<InspectorResizeState, _>(|state, _| {
                state.dragging = false;
            });
        })
}

/// 默认顶栏：标题 + 状态徽标 + 拾取按钮 + 提示（`InspectorPanelSlots::header` 的默认实现）。
///
/// 自定义顶栏可复用本函数包裹扩展（如前面加横幅），或完全自写。
pub fn default_inspector_header(
    inspector: &mut Inspector,
    _window: &mut Window,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let entity = cx.entity();
    let picking = inspector.is_picking();
    let status = if picking {
        "拾取中"
    } else if inspector.active_element_id().is_some() {
        "已选中"
    } else {
        "空闲"
    };
    v_flex()
        .debug_selector(|| "inspector-header".to_string())
        .flex_shrink_0()
        .gap(px(8.0))
        .p(px(12.0))
        .pb(px(4.0))
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(div().text_lg().font_semibold().child("元素检查器"))
                .child(
                    div()
                        .text_xs()
                        .px(px(8.0))
                        .py(px(2.0))
                        .rounded_full()
                        .bg(rgb(0xe8f0fe))
                        .child(status),
                ),
        )
        .child(
            Button::new("inspector-pick")
                .label(if picking {
                    "拾取中，点击画布元素…"
                } else {
                    "拾取元素"
                })
                .primary()
                .on_click(move |_, _, cx| {
                    entity.update(cx, |inspector, _| inspector.start_picking());
                }),
        )
        .child(
            div().text_xs().text_color(rgb(0x888888)).child(
                "悬停高亮蓝框，点击选中元素；重叠处滚轮切换层级。拾取中时画布点击被接管，点任意元素即完成拾取。完整树行点击选中画布对应区域（橙框），箭头逐节点折叠。",
            ),
        )
        .into_any_element()
}

/// 默认段落外皮：白底圆角卡片 + 可选标题行（`InspectorPanelSlots::section` 的默认实现）。
pub fn default_inspector_section(title: Option<SharedString>, body: AnyElement) -> AnyElement {
    v_flex()
        .gap(px(2.0))
        .p(px(8.0))
        .rounded_md()
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(0xe0e0e0))
        .when_some(title, |this, title| {
            this.child(div().text_sm().font_semibold().child(title))
        })
        .child(body)
        .into_any_element()
}

/// 默认的 `DivInspectorState` 检查器展示：布局边界 + 盒模型 + 已指定样式。
///
/// Chrome“元素”面板的对应物：盒模型图（margin/border/padding/content +
/// 实测尺寸）与样式列表（仅显示调用方实际写过的项，未指定的不显示）。
/// 由 `App::enable_default_inspector` 注册；自定义面板沿用本函数即可复用该展示。
pub fn render_div_inspector_state(
    _id: InspectorElementId,
    state: &DivInspectorState,
    _window: &mut Window,
    cx: &mut App,
) -> Div {
    let bounds = state.bounds;
    v_flex()
        .gap(px(2.0))
        .p(px(8.0))
        .rounded_md()
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(0xe0e0e0))
        .child(div().text_sm().font_semibold().child("布局"))
        .child(info_row(
            cx,
            "边界",
            format!(
                "x {:.0} · y {:.0} · w {:.0} · h {:.0}",
                bounds.origin.x.as_f32(),
                bounds.origin.y.as_f32(),
                bounds.size.width.as_f32(),
                bounds.size.height.as_f32()
            ),
        ))
        .child(info_row(
            cx,
            "内容",
            format!(
                "w {:.0} · h {:.0}",
                state.content_size.width.as_f32(),
                state.content_size.height.as_f32()
            ),
        ))
        .child(div().text_sm().font_semibold().mt(px(4.0)).child("盒模型"))
        .child(box_model_diagram(state))
        .child(
            div()
                .text_sm()
                .font_semibold()
                .mt(px(4.0))
                .child("样式（仅已指定项）"),
        )
        .children(specified_style_rows(cx, &state.base_style))
}

/// HSL（0..1）转 sRGB（0..255），颜色可读性用（Chrome 展示 hex 同理）。
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h * 6.0) as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Hsla 可读展示：`#rrggbb`（半透明追加 α）。
fn format_hsla(color: &crate::Hsla) -> String {
    let (r, g, b) = hsl_to_rgb(color.h, color.s, color.l);
    if (color.a - 1.0).abs() < 0.005 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x} · α{:.2}", color.a)
    }
}

/// Fill 可读展示：纯色走 hex，其余原样 `Debug`。
fn format_fill(fill: &crate::Fill) -> String {
    match fill {
        crate::Fill::Color(background) => match background.as_solid() {
            Some(hsla) => format_hsla(&hsla),
            None => format!("{fill:?}"),
        },
    }
}

/// 四边紧凑展示（上 · 右 · 下 · 左，未指定为横线，避免 `EdgesRefinement` 长串溢出）。
fn edges_line<T: std::fmt::Debug>(
    top: &Option<T>,
    right: &Option<T>,
    bottom: &Option<T>,
    left: &Option<T>,
) -> String {
    let one = |v: &Option<T>| {
        v.as_ref()
            .map(|v| format!("{v:?}"))
            .unwrap_or_else(|| "—".to_string())
    };
    format!(
        "{} · {} · {} · {}",
        one(top),
        one(right),
        one(bottom),
        one(left)
    )
}

/// 最近一次点击复制的键（打钩反馈用，存 App 全局）。
#[derive(Default)]
struct CopiedFlash {
    key: String,
}

impl Global for CopiedFlash {}

/// 面板信息行（值可点击复制）。
fn info_row(cx: &mut App, label: &str, value: String) -> impl IntoElement {
    h_flex()
        .gap(px(6.0))
        .child(div().w(px(36.0)).text_xs().child(label.to_string()))
        .child(copyable_value(cx, format!("info:{label}"), value))
}

/// 可复制的值文本（Chrome 点值复制的对应物）。
///
/// 面板文本不可框选（框架限制），统一点击复制 + 打钩反馈；
/// `key` 由调用方保证同屏唯一（标签/属性名天然唯一）。
fn copyable_value(cx: &mut App, key: String, value: String) -> AnyElement {
    let copied = cx.update_default_global::<CopiedFlash, _>(|state, _| state.key == key);
    let value_for_copy = value.clone();
    h_flex()
        .id(format!("copyable-{key}"))
        .flex_1()
        .items_center()
        .gap(px(4.0))
        .rounded_sm()
        .cursor_pointer()
        .hover(|this| this.bg(rgb(0xeeeeee)))
        .on_click(move |_, window, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(value_for_copy.clone()));
            cx.update_default_global::<CopiedFlash, _>(|state, _| {
                state.key = key.clone();
            });
            window.refresh();
        })
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(rgb(0x333333))
                .child(value),
        )
        .when(copied, |this| {
            this.child(div().text_xs().text_color(rgb(0x107c10)).child("✓"))
        })
        .into_any_element()
}

/// 盒模型示意图（Chrome Elements 面板对应物）。
///
/// 四层嵌套示意 + 每层实测/指定数值：margin/border/padding 为指定值
/// （`StyleRefinement` 的 `Debug` 原样展示，未指定显示横线），
/// 内容与边界为实测像素。几何为示意（固定视觉缩进），数字为实数。
fn box_model_diagram(state: &DivInspectorState) -> impl IntoElement {
    let content = format!(
        "{:.0} × {:.0}",
        state.content_size.width.as_f32(),
        state.content_size.height.as_f32()
    );
    let border_box = format!(
        "{:.0} × {:.0}",
        state.bounds.size.width.as_f32(),
        state.bounds.size.height.as_f32()
    );
    let style = &state.base_style;
    v_flex()
        .gap(px(1.0))
        .items_center()
        // margin 层。
        .child(
            v_flex()
                .gap(px(1.0))
                .p(px(6.0))
                .rounded_sm()
                .bg(rgb(0xf9cc9c))
                .child(box_layer_label(
                    "margin",
                    edges_line(
                        &style.margin.top,
                        &style.margin.right,
                        &style.margin.bottom,
                        &style.margin.left,
                    ),
                ))
                // border 层。
                .child(
                    v_flex()
                        .gap(px(1.0))
                        .p(px(6.0))
                        .rounded_sm()
                        .bg(rgb(0xffe188))
                        .child(box_layer_label(
                            "border",
                            edges_line(
                                &style.border_widths.top,
                                &style.border_widths.right,
                                &style.border_widths.bottom,
                                &style.border_widths.left,
                            ),
                        ))
                        // padding 层。
                        .child(
                            v_flex()
                                .gap(px(1.0))
                                .p(px(6.0))
                                .rounded_sm()
                                .bg(rgb(0xc3deb7))
                                .child(box_layer_label(
                                    "padding",
                                    edges_line(
                                        &style.padding.top,
                                        &style.padding.right,
                                        &style.padding.bottom,
                                        &style.padding.left,
                                    ),
                                ))
                                // 内容层（实测）。
                                .child(
                                    div()
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .rounded_sm()
                                        .bg(rgb(0xa4c5f7))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x333333))
                                                .child(format!("content {content}")),
                                        ),
                                ),
                        ),
                ),
        )
        .child(div().text_xs().text_color(rgb(0x888888)).child(format!(
            "border-box {border_box}（几何示意，数字为实数；— 表示未指定）"
        )))
}

/// 盒模型层标签（层名 + 值，单行）。
fn box_layer_label(layer: &str, value: String) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(rgb(0x666666))
        .child(format!("{layer} {value}"))
}

/// 已指定样式列表（Chrome Styles 面板对应物）。
///
/// 只收录调用方实际写过的项（标量 `Option` 取 `Some`，复合 refinement 按子字段
/// 逐项检查），未指定的不显示。颜色走 hex 可读展示，其余值用 `Debug` 原样展示；
/// 每行值可点击复制。
fn specified_style_rows(cx: &mut App, style: &crate::StyleRefinement) -> Vec<AnyElement> {
    // （属性名，值）：`Some` 才收录。
    let mut rows: Vec<(String, String)> = Vec::new();
    macro_rules! specified {
        ($($field:ident),*) => {
            $(
                if let Some(value) = style.$field.as_ref() {
                    rows.push((stringify!($field).to_string(), format!("{value:?}")));
                }
            )*
        };
    }
    specified!(
        display,
        visibility,
        position,
        flex_direction,
        flex_wrap,
        justify_content,
        align_items,
        align_self,
        flex_grow,
        flex_shrink
    );
    // 颜色单独走 hex 可读展示。
    if let Some(fill) = style.background.as_ref() {
        rows.push(("background".to_string(), format_fill(fill)));
    }
    // 复合 refinement：子字段逐项检查（x/y、宽高、四边）。
    macro_rules! specified_sub {
        ($field:ident : $($sub:ident),*) => {
            $(
                if let Some(value) = style.$field.$sub.as_ref() {
                    rows.push((
                        concat!(stringify!($field), ".", stringify!($sub)).to_string(),
                        format!("{value:?}"),
                    ));
                }
            )*
        };
    }
    specified_sub!(overflow: x, y);
    specified_sub!(size: width, height);
    specified_sub!(min_size: width, height);
    specified_sub!(max_size: width, height);
    specified_sub!(gap: width, height);
    specified_sub!(margin: top, right, bottom, left);
    specified_sub!(padding: top, right, bottom, left);
    specified_sub!(border_widths: top, right, bottom, left);
    specified_sub!(inset: top, right, bottom, left);
    if rows.is_empty() {
        return vec![
            div()
                .text_xs()
                .text_color(rgb(0x888888))
                .child("（无显式样式，全走默认值）")
                .into_any_element(),
        ];
    }
    rows.into_iter()
        .map(|(name, value)| {
            h_flex()
                .gap(px(6.0))
                .child(
                    div()
                        .w(px(120.0))
                        .text_xs()
                        .text_color(rgb(0x999999))
                        .child(name.clone()),
                )
                .child(copyable_value(cx, format!("style:{name}"), value))
                .into_any_element()
        })
        .collect()
}

/// 完整树可见行快照（prepaint 记录的 parent→children，DFS 展开）。
struct FullTreeRow {
    /// 节点 id（点击选中用）。
    id: InspectorElementId,
    /// 展开键（`tree_key`）。
    key: String,
    /// 缩进深度。
    depth: usize,
    /// 是否有子节点（决定折叠箭头）。
    has_children: bool,
    /// 当前是否展开。
    expanded: bool,
    /// 是否为当前选中。
    selected: bool,
    /// 单行复制文本（`label #实例 [源码] 实测边界`，无边界为 `?`）。
    copy_text: String,
}

/// 运行卡片（Chrome“性能”面板的轻量对应物）：帧率/帧耗时/CPU/内存/GPU。
///
/// 数据来自采样缓存（仅检查器打开时累计，关闭即停；CPU/内存约 2Hz），
/// 面板只读不测量。采样本身会轻微抬高 CPU 读数，看趋势别看绝对值。
fn runtime_card(window: &Window, cx: &mut App, section: &InspectorSectionSlot) -> AnyElement {
    let fps = window.runtime_fps();
    let frame_ms = window.runtime_frame_ms();
    let cpu = window
        .runtime_cpu()
        .map(|v| format!("{v:.1}%"))
        .unwrap_or_else(|| "—".to_string());
    let mem = window
        .runtime_mem_mb()
        .map(|v| format!("{v:.1} MB"))
        .unwrap_or_else(|| "—".to_string());
    let gpu = crate::runtime_stats::gpu_info()
        .map(|g| format!("{} ({})", g.name, g.backend))
        .unwrap_or_else(|| "未上报（渲染层未注册）".to_string());
    section(
        Some("运行".into()),
        v_flex()
            .gap(px(2.0))
            .child(info_row(
                cx,
                "帧率",
                format!("{fps:.1} FPS · {frame_ms:.1} ms"),
            ))
            .child(info_row(cx, "CPU", cpu))
            .child(info_row(cx, "内存", mem))
            .child(info_row(cx, "GPU", gpu))
            .into_any_element(),
    )
}

/// 报错卡片（Chrome Console 的应用内对应物）：`App::report_error` 上报的错误环。
///
/// 框架不拦截 `log`（应用自有 logger），需要进面板的错误请走 `report_error`；
/// 同一环供崩溃快照读取，死后也有据可查。
fn error_card(cx: &mut Context<Inspector>, section: &InspectorSectionSlot) -> AnyElement {
    let errors = cx.recent_errors();
    let body: AnyElement = if errors.is_empty() {
        div()
            .text_xs()
            .text_color(rgb(0x888888))
            .child("暂无上报错误（`App::report_error` 接入）。")
            .into_any_element()
    } else {
        v_flex()
            .gap(px(1.0))
            .children(errors.iter().rev().take(8).map(|(seq, message)| {
                h_flex()
                    .gap(px(6.0))
                    .child(
                        div()
                            .w(px(36.0))
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child(format!("#{seq}")),
                    )
                    .child(copyable_value(
                        cx,
                        format!("err:{seq}"),
                        message.to_string(),
                    ))
                    .into_any_element()
            }))
            .into_any_element()
    };
    section(Some("报错".into()), body)
}
/// 收集选中元素的祖先键（含自身），用于展开集播种。
fn ancestor_keys(window: &Window, active: &InspectorElementId) -> Vec<String> {
    let mut keys = vec![active.tree_key()];
    let mut current = active.clone();
    while let Some(parent) = window.inspector_tree_parent(&current) {
        keys.push(parent.tree_key());
        current = parent;
    }
    keys
}

/// 快照可见行并计数总数；行数超 cap 时截断渲染但总数照计。
fn snapshot_full_tree(
    window: &Window,
    active: Option<&InspectorElementId>,
    expanded: &HashSet<String>,
    cap: usize,
) -> (Vec<FullTreeRow>, usize) {
    // 实测边界文本（面板单行复制用；画布 hitbox 查不到时为 `?`）。
    let bounds_text_of = |id: &InspectorElementId| {
        window
            .inspector_bounds_for_id(id)
            .or_else(|| window.next_inspector_bounds_for_id(id))
            .map(|b| {
                format!(
                    "{:.0}x{:.0}@({:.0},{:.0})",
                    b.size.width.as_f32(),
                    b.size.height.as_f32(),
                    b.origin.x.as_f32(),
                    b.origin.y.as_f32()
                )
            })
            .unwrap_or_else(|| "?".to_string())
    };
    let mut rows = Vec::new();
    let mut total = 0usize;
    let mut stack: Vec<(InspectorElementId, usize)> = window
        .inspector_tree_roots()
        .into_iter()
        .map(|id| (id, 0))
        .collect();
    stack.reverse();
    // 遍历上限：防止异常大树卡住面板帧（仅检查器打开时执行）。
    let mut visited = 0usize;
    while let Some((id, depth)) = stack.pop() {
        visited += 1;
        if visited > 20000 {
            break;
        }
        total += 1;
        let children = window.inspector_tree_children(&id);
        let key = id.tree_key();
        let is_expanded = expanded.contains(&key);
        if rows.len() < cap {
            let copy_text = format!(
                "{} #{} [{}] {}",
                id.short_label(),
                id.instance_id,
                id.source_label(),
                bounds_text_of(&id)
            );
            rows.push(FullTreeRow {
                selected: active == Some(&id),
                id: id.clone(),
                key: key.clone(),
                depth,
                has_children: !children.is_empty(),
                expanded: is_expanded,
                copy_text,
            });
        }
        if is_expanded {
            for child in children.into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
    }
    (rows, total)
}

/// 完整树卡片：逐节点展开/折叠，点击行选中画布对应区域（外皮走段落插槽）。
fn full_tree_card(
    cx: &mut App,
    rows: Vec<FullTreeRow>,
    total: usize,
    section: &InspectorSectionSlot,
) -> AnyElement {
    // “复制树文本”打钩反馈（与单行复制共用 `CopiedFlash`）。
    let copy_all_done =
        cx.update_default_global::<CopiedFlash, _>(|state, _| state.key == "ftree-copy-all");
    // 单行复制打钩集合：先快照，避免逐行借用 `cx`。
    let copied_keys: HashSet<String> = cx.update_default_global::<CopiedFlash, _>(|state, _| {
        (!state.key.is_empty())
            .then(|| state.key.clone())
            .into_iter()
            .collect()
    });
    let body = v_flex()
        .gap(px(2.0))
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().font_semibold().child("完整树"))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x888888))
                        .child(format!("{total} 个节点")),
                ),
        )
        .child(
            h_flex()
                .gap(px(8.0))
                .child(
                    div()
                        .id("ftree-expand-all")
                        .text_xs()
                        .text_color(rgb(0x666666))
                        .cursor_pointer()
                        .child("全部展开")
                        .on_click(|_, window, cx| {
                            let mut all = Vec::new();
                            let mut stack = window.inspector_tree_roots();
                            while let Some(id) = stack.pop() {
                                all.push(id.tree_key());
                                stack.extend(window.inspector_tree_children(&id));
                            }
                            cx.update_default_global::<FullTreeUiState, _>(|state, _| {
                                state.expanded = all.into_iter().collect();
                            });
                        }),
                )
                .child(
                    div()
                        .id("ftree-collapse-all")
                        .text_xs()
                        .text_color(rgb(0x666666))
                        .cursor_pointer()
                        .child("全部收起")
                        .on_click(|_, _, cx| {
                            cx.update_default_global::<FullTreeUiState, _>(|state, _| {
                                state.expanded.clear();
                            });
                        }),
                )
                .child(
                    div()
                        .id("ftree-copy-text")
                        .text_xs()
                        .text_color(rgb(0x666666))
                        .cursor_pointer()
                        .hover(|this| this.bg(rgb(0xeeeeee)))
                        .child(if copy_all_done {
                            "已复制✓"
                        } else {
                            "复制树文本（喂 AI）"
                        })
                        .on_click(|event, window, cx| {
                            cx.stop_propagation();
                            // AI 可读导出：全量 DFS，不受面板折叠影响。
                            if let Some(text) = window.inspector_tree_text(cx, 2000) {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                                cx.update_default_global::<CopiedFlash, _>(|state, _| {
                                    state.key = "ftree-copy-all".to_string();
                                });
                                window.refresh();
                            }
                            let _ = event;
                        }),
                ),
        )
        .when(rows.is_empty(), |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(rgb(0x888888))
                    .child("暂无树数据：拾取一元素后生成（仅检查器打开时记录）。"),
            )
        })
        .children(rows.into_iter().map(|row| {
            let copied = copied_keys.contains(&format!("ftree-copy-{}", row.key));
            full_tree_row(row, copied)
        }))
        .when(total > FULL_TREE_ROW_CAP, |this| {
            this.child(div().text_xs().text_color(rgb(0x888888)).child(format!(
                "… 仅展示前 {FULL_TREE_ROW_CAP} 行，收起部分节点以精简。"
            )))
        })
        .into_any_element();
    // 完整树自带标题行，外皮不再加标题。
    section(None, body)
}

/// 完整树行：箭头折叠 + 名称选中 + 单行复制，选中行橙底呼应画布高亮。
fn full_tree_row(row: FullTreeRow, copied: bool) -> impl IntoElement {
    let toggle_key = row.key.clone();
    let select_id = row.id.clone();
    let label = row.id.short_label();
    let source = row.id.source_label();
    let instance = row.id.instance_id;
    // 单行复制文本（`label #实例 [源码] 边界`，调试时直接粘给 AI/日志）。
    let copy_text = row.copy_text.clone();
    let copy_key = format!("ftree-copy-{}", row.key);
    let copy_key_for_click = copy_key.clone();
    h_flex()
        .id(format!("ftree-{}", row.key))
        .items_center()
        .gap(px(4.0))
        .ml(px(row.depth as f32 * 12.0))
        .py(px(1.0))
        .px(px(4.0))
        .rounded_sm()
        .when(row.selected, |this| this.bg(rgb(0xfdf0dc)))
        .child(
            div()
                .id(format!("ftree-toggle-{}", row.key))
                .w(px(14.0))
                .text_xs()
                .text_color(rgb(0x666666))
                .cursor_pointer()
                .when(row.has_children, |this| {
                    let key = toggle_key.clone();
                    let arrow = if row.expanded { "▾" } else { "▸" };
                    this.child(arrow).on_click(move |_, _, cx| {
                        cx.update_default_global::<FullTreeUiState, _>(|state, _| {
                            if !state.expanded.remove(&key) {
                                state.expanded.insert(key.clone());
                            }
                        });
                    })
                })
                .when(!row.has_children, |this| {
                    this.child("•").text_color(rgb(0xbbbbbb))
                }),
        )
        .child(
            div()
                .id(format!("ftree-label-{}", row.key))
                .flex_1()
                .text_xs()
                .text_color(rgb(0x333333))
                .cursor_pointer()
                .hover(|this| this.bg(rgb(0xeeeeee)))
                .child(format!("{label} #{instance}"))
                .on_click(move |_, window, cx| {
                    window.select_inspector_element(&select_id, cx);
                }),
        )
        .child(div().text_xs().text_color(rgb(0x999999)).child(source))
        // 单行复制（只复制本行，不干扰行点击选中）。
        .child(
            div()
                .id(format!("ftree-copy-{copy_key}"))
                .text_xs()
                .text_color(if copied { rgb(0x107c10) } else { rgb(0xbbbbbb) })
                .cursor_pointer()
                .hover(|this| this.bg(rgb(0xeeeeee)))
                .child(if copied { "✓" } else { "⧉" })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    window.prevent_default();
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
                    cx.update_default_global::<CopiedFlash, _>(|state, _| {
                        state.key = copy_key_for_click.clone();
                    });
                    window.refresh();
                }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Context, InteractiveElement as _, IntoElement, Modifiers, MouseButton, ParentElement,
        Render, ScrollDelta, ScrollWheelEvent, point, px,
    };

    /// 高宿主视图：60 行 id 元素，撑出超长检查树（内容必高于面板视口）。
    struct TallHost;
    impl Render for TallHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("tall-host")
                .children((0..60).map(|i| div().id(format!("tall-row-{i}"))))
        }
    }

    /// 打开检查器并绘制两帧，返回默认面板宽与布局锚点
    /// （面板/顶栏/滚动区/内容/拖拽条）。
    ///
    /// 树记录只在检查器打开时发生，故先 toggle 再绘制产树，二次绘制展开全树。
    /// 做成宏：展开后的 `VisualTestContext` 留在调用方继续做鼠标/滚轮模拟。
    macro_rules! setup_panel {
        ($cx:expr) => {{
            $cx.update(|cx| {
                crate::theme::init(cx);
                cx.enable_default_inspector();
            });
            let (_view, cx) = $cx.add_window_view(|_, _| TallHost);
            cx.update(|window, cx| {
                window.toggle_inspector(cx);
                let _ = window.draw(cx);
                // 展开全部树节点，造出超长内容。
                let mut stack = window.inspector_tree_roots();
                let mut all = Vec::new();
                while let Some(id) = stack.pop() {
                    all.push(id.tree_key());
                    stack.extend(window.inspector_tree_children(&id));
                }
                cx.update_default_global::<FullTreeUiState, _>(|state, _| {
                    state.expanded = all.into_iter().collect();
                });
                let _ = window.draw(cx);
            });
            let default_width =
                cx.update(|window, _| crate::rems(30.0).to_pixels(window.rem_size()).as_f32());
            let panel = cx.debug_bounds("inspector-panel").unwrap();
            let header = cx.debug_bounds("inspector-header").unwrap();
            let scroll = cx.debug_bounds("inspector-scroll").unwrap();
            let content = cx.debug_bounds("inspector-content").unwrap();
            let strip = cx.debug_bounds("inspector-resize-strip").unwrap();
            (cx, default_width, panel, header, scroll, content, strip)
        }};
    }

    /// 顶栏钉在面板顶部，滚动区填满剩余，拖拽条与面板等高居左，内容超出视口可滚。
    #[crate::test]
    fn panel_layout_pins_header_and_scroll(cx: &mut crate::TestAppContext) {
        let (_cx, _default_width, panel, header, scroll, content, strip) = setup_panel!(cx);
        let eps = 1.0;
        // 顶栏钉住顶部。
        assert!(
            (header.origin.y.as_f32() - panel.origin.y.as_f32()).abs() < eps,
            "顶栏应钉在面板顶部：{header:?} vs {panel:?}"
        );
        // 滚动区从顶栏底部开始，到面板底部结束。
        assert!(
            (scroll.origin.y.as_f32() - (header.origin.y.as_f32() + header.size.height.as_f32()))
                .abs()
                < eps + 12.0,
            "滚动区应接顶栏底部：{scroll:?} vs {header:?}"
        );
        assert!(
            ((scroll.origin.y.as_f32() + scroll.size.height.as_f32())
                - (panel.origin.y.as_f32() + panel.size.height.as_f32()))
            .abs()
                < eps,
            "滚动区应填满面板剩余高度：{scroll:?} vs {panel:?}"
        );
        // 拖拽条居左等高。
        assert!(
            (strip.origin.x.as_f32() - panel.origin.x.as_f32()).abs() < eps,
            "拖拽条应在面板左缘：{strip:?} vs {panel:?}"
        );
        assert!(
            (strip.size.height.as_f32() - panel.size.height.as_f32()).abs() < eps,
            "拖拽条应与面板等高：{strip:?} vs {panel:?}"
        );
        // 内容超出滚动视口（可滚）。
        assert!(
            content.size.height.as_f32() > scroll.size.height.as_f32(),
            "内容应超出滚动视口：{content:?} vs {scroll:?}"
        );
    }

    /// 滚轮滚动后顶栏位置不变（sticky 回归）。
    #[crate::test]
    fn wheel_keeps_header_pinned(cx: &mut crate::TestAppContext) {
        let (cx, _default_width, _panel, header_before, scroll, _content, _strip) =
            setup_panel!(cx);
        let center = crate::Point {
            x: px(scroll.origin.x.as_f32() + scroll.size.width.as_f32() / 2.0),
            y: px(scroll.origin.y.as_f32() + scroll.size.height.as_f32() / 2.0),
        };
        cx.simulate_event(ScrollWheelEvent {
            position: center,
            delta: ScrollDelta::Lines(point(0.0f32, 5.0)),
            modifiers: Modifiers::default(),
            ..Default::default()
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let header_after = cx.debug_bounds("inspector-header").unwrap();
        assert!(
            (header_after.origin.y.as_f32() - header_before.origin.y.as_f32()).abs() < 1.0,
            "滚轮后顶栏应钉住不动：{header_before:?} vs {header_after:?}"
        );
    }

    /// 拖拽条拖动改面板宽（按下 → 左移 100px → 松开）。
    #[crate::test]
    fn drag_strip_resizes_panel(cx: &mut crate::TestAppContext) {
        let (cx, default_width, _panel, _header, _scroll, _content, strip) = setup_panel!(cx);
        // 新开检查器默认拾取中（鼠标事件被拾取接管）：先选中退出拾取，再拖拽。
        cx.update(|window, cx| {
            let roots = window.inspector_tree_roots();
            assert!(!roots.is_empty());
            assert!(window.select_inspector_element(&roots[0], cx));
            assert!(!window.is_inspector_picking(cx));
            let _ = window.draw(cx);
        });
        let start_x = strip.origin.x.as_f32() + strip.size.width.as_f32() / 2.0;
        let start_y = strip.origin.y.as_f32() + strip.size.height.as_f32() / 2.0;
        // 注意：hover 快照只在 draw 时刷新，每步事件后都补一帧（与真实主循环一致）。
        cx.simulate_mouse_move(
            crate::Point {
                x: px(start_x),
                y: px(start_y),
            },
            None,
            Modifiers::default(),
        );
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.simulate_mouse_down(
            crate::Point {
                x: px(start_x),
                y: px(start_y),
            },
            MouseButton::Left,
            Modifiers::default(),
        );
        // 条内小幅移动：验证 handler 本体（悬停完好）。
        cx.simulate_mouse_move(
            crate::Point {
                x: px(start_x - 2.0),
                y: px(start_y + 2.0),
            },
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        // 条外大幅移动：验证指针捕获（移出条外仍跟进）。
        cx.simulate_mouse_move(
            crate::Point {
                x: px(start_x - 100.0),
                y: px(start_y),
            },
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.simulate_mouse_up(
            crate::Point {
                x: px(start_x - 100.0),
                y: px(start_y),
            },
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.update(|window, _| {
            let width = window
                .inspector_width()
                .expect("拖拽后应有自定义宽度")
                .as_f32();
            assert!(
                (width - (default_width + 100.0)).abs() < 2.0,
                "左移 100px 应加宽 100px：{width} vs {}",
                default_width + 100.0
            );
        });
    }

    /// 拾取态下拖拽条仍可拖（不退出拾取）：打开即拾取中，直接拖左移 60px。
    #[crate::test]
    fn drag_strip_works_while_picking(cx: &mut crate::TestAppContext) {
        let (cx, default_width, _panel, _header, _scroll, _content, strip) = setup_panel!(cx);
        cx.update(|window, cx| {
            assert!(window.is_inspector_picking(cx), "新开检查器应默认拾取中");
        });
        let start_x = strip.origin.x.as_f32() + strip.size.width.as_f32() / 2.0;
        let start_y = strip.origin.y.as_f32() + strip.size.height.as_f32() / 2.0;
        cx.simulate_mouse_move(
            crate::Point {
                x: px(start_x),
                y: px(start_y),
            },
            None,
            Modifiers::default(),
        );
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.simulate_mouse_down(
            crate::Point {
                x: px(start_x),
                y: px(start_y),
            },
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            crate::Point {
                x: px(start_x - 60.0),
                y: px(start_y),
            },
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.simulate_mouse_up(
            crate::Point {
                x: px(start_x - 60.0),
                y: px(start_y),
            },
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.update(|window, cx| {
            let width = window
                .inspector_width()
                .expect("拾取态拖拽后应有自定义宽度")
                .as_f32();
            assert!(
                (width - (default_width + 60.0)).abs() < 2.0,
                "拾取态左移 60px 应加宽 60px：{width} vs {}",
                default_width + 60.0
            );
            assert!(window.is_inspector_picking(cx), "拖拽不应顺手退出拾取");
        });
    }

    /// 面板区滚轮不切换拾取层级（滚面板内容，拾取深度不变）。
    #[crate::test]
    fn wheel_over_panel_keeps_pick_depth(cx: &mut crate::TestAppContext) {
        let (cx, _default_width, panel, _header, scroll, _content, _strip) = setup_panel!(cx);
        cx.update(|window, cx| {
            assert!(window.is_inspector_picking(cx));
        });
        let center = crate::Point {
            x: px(scroll.origin.x.as_f32() + scroll.size.width.as_f32() / 2.0),
            y: px(scroll.origin.y.as_f32() + scroll.size.height.as_f32() / 2.0),
        };
        // 面板中心确在面板区域内（拾取让路的前提）。
        assert!(
            center.x.as_f32() >= panel.origin.x.as_f32(),
            "面板滚动区中心应在面板内：{center:?} vs {panel:?}"
        );
        cx.simulate_event(ScrollWheelEvent {
            position: center,
            delta: ScrollDelta::Lines(point(0.0f32, 5.0)),
            modifiers: Modifiers::default(),
            ..Default::default()
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            // 滚轮落在面板上：不应触发画布拾取层级切换（active 仍空）。
            assert!(window.is_inspector_picking(cx), "面板滚轮不应退出拾取");
        });
    }

    /// 完整树行自带可复制文本（标签/实例/源码/边界齐全，调试可粘）。
    #[crate::test]
    fn full_tree_rows_carry_copy_text(cx: &mut crate::TestAppContext) {
        let (cx, _, _, _, _, _, _) = setup_panel!(cx);
        cx.update(|window, _| {
            let (rows, total) =
                snapshot_full_tree(window, None, &HashSet::new(), FULL_TREE_ROW_CAP);
            assert!(!rows.is_empty() && total > 0, "应有树行快照");
            let first = &rows[0];
            assert!(
                first.copy_text.contains(&first.id.short_label()),
                "复制文本应含标签：{}",
                first.copy_text
            );
            assert!(
                first.copy_text.contains(&first.id.source_label()),
                "复制文本应含源码位置：{}",
                first.copy_text
            );
        });
    }
}
