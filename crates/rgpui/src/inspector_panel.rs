//! 检查器默认面板（I3 开箱即用）。
//!
//! `App::enable_default_inspector` 一行启用本面板（含 `Div` 布局展示注册）；
//! 应用入口再调 `window.toggle_inspector(cx)` 即出完整面板，共两行。
//! 自定义时整板替换（见 `custom` recipe：复用 [`default_inspector_panel`] 包裹横幅）
//! 或按状态类型扩展（`register_inspector_element`，默认已注册 Div 展示）。

use crate::{
    AnyElement, App, Button, ButtonVariants, Context, Div, DivInspectorState, Global, Inspector,
    InspectorElementId, IntoElement, ParentElement, SharedString, Window, div, h_flex, prelude::*,
    px, rgb, v_flex,
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
        .size_full()
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
                        .flex_1()
                        .overflow_scroll()
                        .child(
                            v_flex()
                                .gap(px(8.0))
                                .p(px(12.0))
                                .pt(px(4.0))
                                .when_some(selected, |this, (loc, instance)| {
                                    this.child(section(
                                        Some("选中元素".into()),
                                        v_flex()
                                            .gap(px(2.0))
                                            .child(info_row("源码", loc))
                                            .child(info_row("实例", format!("#{instance}")))
                                            .into_any_element(),
                                    ))
                                })
                                .child(full_tree_card(tree_rows, tree_total, &section))
                                .child(runtime_card(window, &section))
                                .child(error_card(cx, &section))
                                .children(states),
                        ),
                ),
        )
        .into_any_element()
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
    _cx: &mut App,
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
        .children(specified_style_rows(&state.base_style))
}

/// 面板信息行辅助函数。
fn info_row(label: &str, value: String) -> impl IntoElement {
    h_flex()
        .gap(px(6.0))
        .child(div().w(px(36.0)).text_xs().child(label.to_string()))
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(rgb(0x333333))
                .child(value),
        )
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
                .child(box_layer_label("margin", format!("{:?}", style.margin)))
                // border 层。
                .child(
                    v_flex()
                        .gap(px(1.0))
                        .p(px(6.0))
                        .rounded_sm()
                        .bg(rgb(0xffe188))
                        .child(box_layer_label(
                            "border",
                            format!("{:?}", style.border_widths),
                        ))
                        // padding 层。
                        .child(
                            v_flex()
                                .gap(px(1.0))
                                .p(px(6.0))
                                .rounded_sm()
                                .bg(rgb(0xc3deb7))
                                .child(box_layer_label("padding", format!("{:?}", style.padding)))
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
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x888888))
                .child(format!("border-box {border_box}（几何示意，数字为实数）")),
        )
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
/// 逐项检查），未指定的不显示。值用 `Debug` 原样展示。
fn specified_style_rows(style: &crate::StyleRefinement) -> Vec<AnyElement> {
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
        flex_shrink,
        background
    );
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
                        .w(px(88.0))
                        .text_xs()
                        .text_color(rgb(0x999999))
                        .child(name),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(rgb(0x333333))
                        .child(value),
                )
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
}

/// 运行卡片（Chrome“性能”面板的轻量对应物）：帧率/帧耗时/CPU/内存/GPU。
///
/// 数据来自采样缓存（仅检查器打开时累计，关闭即停；CPU/内存约 2Hz），
/// 面板只读不测量。采样本身会轻微抬高 CPU 读数，看趋势别看绝对值。
fn runtime_card(window: &Window, section: &InspectorSectionSlot) -> AnyElement {
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
            .child(info_row("帧率", format!("{fps:.1} FPS · {frame_ms:.1} ms")))
            .child(info_row("CPU", cpu))
            .child(info_row("内存", mem))
            .child(info_row("GPU", gpu))
            .into_any_element(),
    )
}

/// 报错卡片（Chrome Console 的应用内对应物）：`App::report_error` 上报的错误环。
///
/// 框架不拦截 `log`（应用自有 logger），需要进面板的错误请走 `report_error`；
/// 同一环供崩溃快照读取，死后也有据可查。
fn error_card(cx: &Context<Inspector>, section: &InspectorSectionSlot) -> AnyElement {
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
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(rgb(0xb91c1c))
                            .child(message.clone()),
                    )
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
            rows.push(FullTreeRow {
                selected: active == Some(&id),
                id: id.clone(),
                key: key.clone(),
                depth,
                has_children: !children.is_empty(),
                expanded: is_expanded,
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
    rows: Vec<FullTreeRow>,
    total: usize,
    section: &InspectorSectionSlot,
) -> AnyElement {
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
        .children(rows.into_iter().map(full_tree_row))
        .when(total > FULL_TREE_ROW_CAP, |this| {
            this.child(div().text_xs().text_color(rgb(0x888888)).child(format!(
                "… 仅展示前 {FULL_TREE_ROW_CAP} 行，收起部分节点以精简。"
            )))
        })
        .into_any_element();
    // 完整树自带标题行，外皮不再加标题。
    section(None, body)
}

/// 完整树行：箭头折叠 + 名称选中，选中行橙底呼应画布高亮。
fn full_tree_row(row: FullTreeRow) -> impl IntoElement {
    let toggle_key = row.key.clone();
    let select_id = row.id.clone();
    let label = row.id.short_label();
    let source = row.id.source_label();
    let instance = row.id.instance_id;
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
}
