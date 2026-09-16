//! 检查器默认面板（I3 开箱即用）。
//!
//! `App::enable_default_inspector` 一行启用本面板（含 `Div` 布局展示注册）；
//! 应用入口再调 `window.toggle_inspector(cx)` 即出完整面板，共两行。
//! 自定义时整板替换（见 `custom` recipe：复用 [`default_inspector_panel`] 包裹横幅）
//! 或按状态类型扩展（`register_inspector_element`，默认已注册 Div 展示）。

use crate::{
    AnyElement, App, Button, ButtonVariants, ClipboardItem, Context, Div, DivInspectorState,
    Global, Inspector, InspectorElementId, IntoElement, ParentElement, Window, div, h_flex,
    prelude::*, px, rgb, v_flex,
};
use std::collections::HashSet;

/// 祖先链超过该层级数时折叠中间层。
const SPINE_COLLAPSE_THRESHOLD: usize = 8;
/// 折叠时保留的头部层级数。
const SPINE_HEAD_KEPT: usize = 2;
/// 折叠时保留的尾部层级数。
const SPINE_TAIL_KEPT: usize = 3;
/// 完整树单帧最多渲染行数（超量截断并提示，面板外层已可滚动）。
const FULL_TREE_ROW_CAP: usize = 800;

/// 祖先链折叠/复制的面板本地 UI 状态（存 App 全局）。
#[derive(Default)]
struct SpineUiState {
    /// 当前选中元素的标识（路径 + 实例），变化时重置折叠/复制态。
    selected_key: String,
    /// 长链是否展开。
    expanded: bool,
    /// 已复制路径的层级索引。
    copied: Option<usize>,
}

impl Global for SpineUiState {}

/// 完整树逐节点展开状态（存 App 全局；选中变化时以祖先链重播种）。
#[derive(Default)]
struct FullTreeUiState {
    /// 播种时的选中键，变化即重播种。
    selected_key: String,
    /// 已展开节点的稳定键集合。
    expanded: HashSet<String>,
}

impl Global for FullTreeUiState {}

/// 默认检查器面板：状态徽标、拾取按钮、源码位置、祖先链树、完整树与已注册状态展示。
///
/// 经 `App::enable_default_inspector` 注册；自定义面板可直接复用本函数包裹扩展。
pub fn default_inspector_panel(
    inspector: &mut Inspector,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let entity = cx.entity();
    let picking = inspector.is_picking();
    // 先物化选中信息为 owned 数据，结束借用后再渲染各状态。
    // 祖先链：GlobalElementId 本身就是从根到选中元素的 id 路径。
    let selected: Option<(Vec<String>, String, usize)> = inspector.active_element_id().map(|id| {
        let loc = id.path.source_location;
        let spine = id.path.global_id.iter().map(|e| e.to_string()).collect();
        (
            spine,
            format!("{}:{}", loc.file(), loc.line()),
            id.instance_id,
        )
    });
    let status = if picking {
        "拾取中"
    } else if selected.is_some() {
        "已选中"
    } else {
        "空闲"
    };
    // 同步祖先链 UI 状态：选中变化时重置折叠/复制态。
    let (expanded, copied) = cx.update_default_global::<SpineUiState, _>(|state, _| {
        let key = selected
            .as_ref()
            .map(|(spine, _, instance)| format!("{}#{instance}", spine.join(".")))
            .unwrap_or_default();
        if state.selected_key != key {
            state.selected_key = key;
            state.expanded = false;
            state.copied = None;
        }
        (state.expanded, state.copied)
    });
    // 完整树展开集：选中变化时以祖先链重播种，其后用户可自由折叠。
    let active_id: Option<InspectorElementId> = inspector.active_element_id().cloned();
    let active_key = active_id
        .as_ref()
        .map(|id| id.tree_key())
        .unwrap_or_default();
    let tree_expanded = cx.update_default_global::<FullTreeUiState, _>(|state, _| {
        if state.selected_key != active_key {
            state.selected_key = active_key.clone();
            state.expanded = active_id
                .as_ref()
                .map(|id| ancestor_keys(window, id))
                .unwrap_or_default()
                .into_iter()
                .collect();
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

    div()
        .id("inspector-panel")
        .size_full()
        .overflow_scroll()
        .bg(rgb(0xf7f7f7))
        .child(
            v_flex()
                .gap(px(8.0))
                .p(px(12.0))
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
                        "悬停高亮蓝框，点击选中元素；重叠处滚轮切换层级。拾取中时画布点击被接管，点任意元素即完成拾取。树节点点击选中画布对应祖先区域（橙框）并复制该层路径，超长链自动折叠。",
                    ),
                )
                .when_some(selected, |this, (spine, loc, instance)| {
                    this.child(
                        v_flex()
                            .gap(px(2.0))
                            .p(px(8.0))
                            .rounded_md()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xe0e0e0))
                            .child(div().text_sm().font_semibold().child("选中元素"))
                            .child(info_row("源码", loc))
                            .child(info_row("实例", format!("#{instance}")))
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .mt(px(4.0))
                                    .child("元素树（祖先链，点击节点选中画布对应区域）"),
                            )
                            .child(element_spine(spine, expanded, copied)),
                    )
                })
                .child(full_tree_card(tree_rows, tree_total))
                .children(states),
        )
        .into_any_element()
}

/// 默认的 `DivInspectorState` 检查器展示：被选中 Div 的布局边界与内容尺寸。
///
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

/// 祖先链渲染为缩进树（根在上、选中在下）。
///
/// 超长链自动折叠中间层；点击节点选中画布对应祖先区域并复制该层完整路径，
/// 点击折叠行展开/收起。
fn element_spine(spine: Vec<String>, expanded: bool, copied: Option<usize>) -> impl IntoElement {
    let depth = spine.len();
    let collapsible = depth > SPINE_COLLAPSE_THRESHOLD;
    // 折叠时仅保留首尾若干层。
    let visible: Vec<(usize, &String)> = if collapsible && !expanded {
        spine
            .iter()
            .enumerate()
            .take(SPINE_HEAD_KEPT)
            .chain(spine.iter().enumerate().skip(depth - SPINE_TAIL_KEPT))
            .collect()
    } else {
        spine.iter().enumerate().collect()
    };
    v_flex()
        .gap(px(1.0))
        .children(visible.into_iter().map(|(ix, name)| {
            let last = ix + 1 == depth;
            let path_to_here = spine[..=ix].join(".");
            let levels_up = depth.saturating_sub(ix + 1);
            spine_row(ix, name, last, copied == Some(ix), path_to_here, levels_up)
        }))
        // 折叠行：展示被隐藏的层级数，点击展开。
        .when(collapsible && !expanded, |this| {
            this.child(
                div()
                    .id("spine-toggle")
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .ml(px(SPINE_HEAD_KEPT as f32 * 12.0))
                    .py(px(2.0))
                    .cursor_pointer()
                    .child(format!(
                        "⋯ {} 级已折叠，点击展开",
                        depth - SPINE_HEAD_KEPT - SPINE_TAIL_KEPT
                    ))
                    .on_click(|_, _, cx| {
                        cx.update_default_global::<SpineUiState, _>(|state, _| {
                            state.expanded = true;
                        });
                    }),
            )
        })
        // 展开态收起行。
        .when(collapsible && expanded, |this| {
            this.child(
                div()
                    .id("spine-toggle")
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .py(px(2.0))
                    .cursor_pointer()
                    .child("收起，点击折叠")
                    .on_click(|_, _, cx| {
                        cx.update_default_global::<SpineUiState, _>(|state, _| {
                            state.expanded = false;
                        });
                    }),
            )
        })
        .when(depth == 0, |this| {
            this.child(div().text_xs().child("(根元素，无祖先路径)"))
        })
}

/// 祖先链节点行：缩进 + 连接符 + 名称，点击选中画布对应祖先区域并复制路径。
fn spine_row(
    ix: usize,
    name: &str,
    last: bool,
    is_copied: bool,
    path_to_here: String,
    levels_up: usize,
) -> impl IntoElement {
    h_flex()
        .id(format!("spine-node-{ix}"))
        .items_center()
        .gap(px(4.0))
        .ml(px(ix as f32 * 12.0))
        .py(px(1.0))
        .px(px(4.0))
        .rounded_sm()
        .cursor_pointer()
        .hover(|this| this.bg(rgb(0xeeeeee)))
        .when(last, |this| this.bg(rgb(0xe8f0fe)))
        .on_click(move |_, window, cx| {
            // 双向映射：树点击 → 画布祖先区域高亮 + 面板显示该节点源码与边界。
            window.select_inspector_ancestor(levels_up, cx);
            cx.write_to_clipboard(ClipboardItem::new_string(path_to_here.clone()));
            cx.update_default_global::<SpineUiState, _>(|state, _| {
                state.copied = Some(ix);
            });
        })
        .child(div().text_xs().text_color(rgb(0x999999)).child(if last {
            "└─"
        } else {
            "├─"
        }))
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(rgb(0x333333))
                .child(if name.is_empty() {
                    "(空)".to_string()
                } else {
                    name.to_string()
                }),
        )
        .when(is_copied, |this| {
            this.child(div().text_xs().text_color(rgb(0x107c10)).child("✓"))
        })
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

/// 完整树卡片：逐节点展开/折叠，点击行选中画布对应区域。
fn full_tree_card(rows: Vec<FullTreeRow>, total: usize) -> impl IntoElement {
    v_flex()
        .gap(px(2.0))
        .p(px(8.0))
        .rounded_md()
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(0xe0e0e0))
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
