//! 画布语义（Canvas Accessibility Semantics）。
//!
//! 自绘组件（电子表格网格、页视图、墨迹、图表、时间线、MDI 子窗口…）用
//! [`Window::paint_*`][crate::Window] 直接画像素，元素树里只有**一个**叶子节点 ——
//! 于是屏幕阅读器看不到「一格单元格」「一笔笔迹」。本模块补上这段：把画布上的每个
//! 自绘对象描述成 [`CanvasNode`]，交给 [`Element::a11y_synthetic_children`] 发布
//! 成合成子节点，并把辅助技术派发到这些子节点的动作路由回应用。
//!
//! 用到的既有设施：
//! - [`CanvasSemantics`]：本帧的语义节点集合（绘制顺序 = 叠放顺序），提供阅读顺序、
//!   命中测试、局部 → 窗口矩形换算；
//! - [`push_canvas_a11y`]：在 `a11y_synthetic_children` 里发布合成子节点（带矩形，
//!   因此内置的 `Click` 回退也能作用于它们）；
//! - [`register_canvas_a11y_actions`]：在 `paint` 里为这些节点登记动作监听器；
//! - [`crate::elements::canvas`]：{@link Canvas} 元素把这套接线打包成
//!   `.id(..)` / `.a11y(..)` / `.on_canvas_a11y_action(..)` 三个 builder。
//!
//! ```
//! use rgpui::canvas_a11y::{CanvasNode, CanvasSemantics};
//! use rgpui::{Bounds, Role, point, px, size};
//!
//! let mut semantics = CanvasSemantics::new();
//! semantics.push(CanvasNode::new(
//!     1,
//!     Role::GraphicsObject,
//!     "矩形 A",
//!     Bounds::new(point(px(10.0), px(20.0)), size(px(80.0), px(40.0))),
//! )
//! .with_actions([rgpui::AccessibleAction::Click]));
//!
//! // 阅读顺序：先按行（y 容差内），行内按 x。
//! assert_eq!(semantics.reading_order(), vec![1]);
//! // 命中测试：后画者在上。
//! assert_eq!(
//!     semantics.hit_test(point(px(40.0), px(30.0))),
//!     Some(1)
//! );
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use accesskit::{Action, ActionData, Role};

use crate::{
    A11ySubtreeBuilder, App, Bounds, GlobalElementId, Pixels, Point, SharedString, Window, px,
    synthetic_child_node_id,
};

/// 画布语义节点的稳定标识。
///
/// 由应用自定义：同一个自绘对象跨帧使用同一 id，辅助技术才会把它视为「同一个」节点
/// （否则会被当成「删掉一个、新增一个」，从而反复播报）。
pub type CanvasNodeId = u64;

/// 阅读顺序分行时的默认 y 容差：同一行内 y 差不超过该值的节点按 x 排序。
pub const READING_ROW_TOLERANCE: Pixels = px(4.0);

/// 画布上的一个语义节点（一个自绘对象）。
///
/// `bounds` 是**画布局部坐标**；发布到无障碍树时由 [`push_canvas_a11y`] 加上画布
/// 原点换算成窗口坐标。
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasNode {
    /// 应用自定义的稳定标识。
    id: CanvasNodeId,
    /// 无障碍角色（如 [`Role::GraphicsObject`]、[`Role::ListItem`]、[`Role::Cell`]）。
    role: Role,
    /// 无障碍名称（屏幕阅读器播报的文本）。
    label: SharedString,
    /// 画布局部坐标下的矩形。
    bounds: Bounds<Pixels>,
    /// 值（如单元格内容、滑块数值）。
    value: Option<SharedString>,
    /// 补充说明。
    description: Option<SharedString>,
    /// 是否禁用。
    disabled: bool,
    /// 是否选中。
    selected: bool,
    /// 是否是当前的键盘 / 无障碍焦点对象。
    focused: bool,
    /// 声明支持的动作（会写入无障碍节点，并由 [`register_canvas_a11y_actions`] 登记监听器）。
    actions: Vec<Action>,
}

impl CanvasNode {
    /// 创建一个语义节点。
    pub fn new(
        id: CanvasNodeId,
        role: Role,
        label: impl Into<SharedString>,
        bounds: Bounds<Pixels>,
    ) -> Self {
        Self {
            id,
            role,
            label: label.into(),
            bounds,
            value: None,
            description: None,
            disabled: false,
            selected: false,
            focused: false,
            actions: Vec::new(),
        }
    }

    /// 设置值。
    pub fn with_value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// 设置补充说明。
    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置禁用状态。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置选中状态。
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// 设置为当前焦点对象（发布时会同时登记为父节点的 active descendant）。
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// 追加一个支持的动作。
    pub fn with_action(mut self, action: Action) -> Self {
        if !self.actions.contains(&action) {
            self.actions.push(action);
        }
        self
    }

    /// 追加若干支持的动作。
    pub fn with_actions(mut self, actions: impl IntoIterator<Item = Action>) -> Self {
        for action in actions {
            self = self.with_action(action);
        }
        self
    }

    /// 标识。
    pub fn id(&self) -> CanvasNodeId {
        self.id
    }

    /// 角色。
    pub fn role(&self) -> Role {
        self.role
    }

    /// 名称。
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// 画布局部坐标下的矩形。
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// 值。
    pub fn value(&self) -> Option<&SharedString> {
        self.value.as_ref()
    }

    /// 补充说明。
    pub fn description(&self) -> Option<&SharedString> {
        self.description.as_ref()
    }

    /// 是否禁用。
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// 是否选中。
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// 是否是当前焦点对象。
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// 声明支持的动作。
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// 就地设置选中状态。
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// 就地设置焦点状态。
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// 就地设置禁用状态。
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }
}

/// 画布语义集合：按绘制顺序存放（**先画的在前**，后画者在叠放上层）。
///
/// 应用每帧重建即可（对象数量通常不大）；命中测试按逆绘制序进行，与视觉叠放一致。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanvasSemantics {
    nodes: Vec<CanvasNode>,
    row_tolerance: Option<Pixels>,
}

impl CanvasSemantics {
    /// 创建空集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置阅读顺序分行时的 y 容差。
    pub fn with_row_tolerance(mut self, tolerance: Pixels) -> Self {
        self.row_tolerance = Some(tolerance);
        self
    }

    /// 分行容差（默认 [`READING_ROW_TOLERANCE`]）。
    pub fn row_tolerance(&self) -> Pixels {
        self.row_tolerance.unwrap_or(READING_ROW_TOLERANCE)
    }

    /// 清空集合（保留容差设置）。
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// 追加一个节点。
    pub fn push(&mut self, node: CanvasNode) {
        self.nodes.push(node);
    }

    /// 追加若干节点。
    pub fn extend(&mut self, nodes: impl IntoIterator<Item = CanvasNode>) {
        self.nodes.extend(nodes);
    }

    /// 节点数量。
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 节点列表（绘制顺序）。
    pub fn nodes(&self) -> &[CanvasNode] {
        &self.nodes
    }

    /// 迭代节点（绘制顺序）。
    pub fn iter(&self) -> std::slice::Iter<'_, CanvasNode> {
        self.nodes.iter()
    }

    /// 按标识取节点。
    pub fn get(&self, id: CanvasNodeId) -> Option<&CanvasNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// 按标识取节点（可变），便于就地改选中 / 焦点状态。
    pub fn get_mut(&mut self, id: CanvasNodeId) -> Option<&mut CanvasNode> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    /// 标识在集合中的下标（绘制顺序）。
    pub fn index_of(&self, id: CanvasNodeId) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }

    /// 命中测试：返回该点下**最上层**（最后绘制）的对象。
    pub fn hit_test(&self, point: Point<Pixels>) -> Option<CanvasNodeId> {
        node_at_point(&self.nodes, point).map(|index| self.nodes[index].id)
    }

    /// 阅读顺序（先按行，行内按 x）的标识列表。
    pub fn reading_order(&self) -> Vec<CanvasNodeId> {
        reading_order_indices(&self.nodes, self.row_tolerance())
            .into_iter()
            .map(|index| self.nodes[index].id)
            .collect()
    }

    /// 阅读顺序中的第一个对象。
    pub fn first_in_reading_order(&self) -> Option<CanvasNodeId> {
        self.reading_order().first().copied()
    }

    /// 阅读顺序中的下一个对象（`forward` 为假时上一个；两端环绕）。
    ///
    /// `current` 为 `None`（或不在集合中）时返回首 / 末个，便于键盘导航起步。
    pub fn next_in_reading_order(
        &self,
        current: Option<CanvasNodeId>,
        forward: bool,
    ) -> Option<CanvasNodeId> {
        let order: Vec<usize> = reading_order_indices(&self.nodes, self.row_tolerance());
        let current_index = current.and_then(|id| self.index_of(id));
        next_in_order(&order, current_index, forward).map(|index| self.nodes[index].id)
    }

    /// 把节点的局部矩形换算为窗口矩形。
    pub fn window_bounds(
        &self,
        canvas_origin: Point<Pixels>,
        id: CanvasNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.get(id).map(|node| window_bounds(canvas_origin, node))
    }
}

/// 局部矩形 → 窗口矩形。
pub fn window_bounds(canvas_origin: Point<Pixels>, node: &CanvasNode) -> Bounds<Pixels> {
    Bounds::new(
        Point::new(
            canvas_origin.x + node.bounds.origin.x,
            canvas_origin.y + node.bounds.origin.y,
        ),
        node.bounds.size,
    )
}

/// 命中测试：返回下标（**逆绘制序**，即后画者优先）。
pub fn node_at_point(nodes: &[CanvasNode], point: Point<Pixels>) -> Option<usize> {
    nodes.iter().rposition(|node| node.bounds.contains(&point))
}

/// 阅读顺序下标：先按 y 分行（行内 y 差不超过 `row_tolerance`），行内按 x 升序。
///
/// 分行是两趟排序（先整体按 y，再逐行按 x），避免使用带容差的比较函数 —— 那种
/// 比较不满足全序，标准库排序可能因此 panic。
pub fn reading_order_indices(nodes: &[CanvasNode], row_tolerance: Pixels) -> Vec<usize> {
    let mut by_y: Vec<usize> = (0..nodes.len()).collect();
    by_y.sort_by(|&a, &b| {
        nodes[a]
            .bounds
            .origin
            .y
            .cmp(&nodes[b].bounds.origin.y)
            .then_with(|| nodes[a].bounds.origin.x.cmp(&nodes[b].bounds.origin.x))
            .then(a.cmp(&b))
    });

    let mut order = Vec::with_capacity(nodes.len());
    let mut row_start = 0;
    while row_start < by_y.len() {
        let row_y = nodes[by_y[row_start]].bounds.origin.y;
        let mut row_end = row_start;
        while row_end < by_y.len() && nodes[by_y[row_end]].bounds.origin.y - row_y <= row_tolerance
        {
            row_end += 1;
        }
        let mut row = by_y[row_start..row_end].to_vec();
        row.sort_by(|&a, &b| {
            nodes[a]
                .bounds
                .origin
                .x
                .cmp(&nodes[b].bounds.origin.x)
                .then(a.cmp(&b))
        });
        order.extend(row);
        row_start = row_end;
    }
    order
}

/// 在阅读顺序中前后移动（下标版；两端环绕，`current` 为 `None` 时回到首 / 末）。
pub fn next_in_order(order: &[usize], current: Option<usize>, forward: bool) -> Option<usize> {
    if order.is_empty() {
        return None;
    }
    let position =
        match current.and_then(|current| order.iter().position(|&index| index == current)) {
            Some(position) => position,
            None => {
                return if forward {
                    order.first().copied()
                } else {
                    order.last().copied()
                };
            }
        };
    let next = if forward {
        (position + 1) % order.len()
    } else {
        (position + order.len() - 1) % order.len()
    };
    order.get(next).copied()
}

/// 把一个语义节点写成 AccessKit 节点。
///
/// `window_bounds` 是窗口坐标下的矩形；`scale_factor` 用于换算成 AccessKit 要求的
/// 物理像素（框架发布真实元素节点时同样乘以缩放因子）。
pub fn accesskit_node(
    node: &CanvasNode,
    window_bounds: Bounds<Pixels>,
    scale_factor: f32,
) -> accesskit::Node {
    let mut out = accesskit::Node::new(node.role);
    out.set_label(node.label.to_string());
    if let Some(value) = &node.value {
        out.set_value(value.to_string());
    }
    if let Some(description) = &node.description {
        out.set_description(description.to_string());
    }
    if node.disabled {
        out.set_disabled();
    }
    if node.selected {
        out.set_selected(true);
    }
    out.set_bounds(accesskit::Rect {
        x0: (window_bounds.origin.x.0 * scale_factor) as f64,
        y0: (window_bounds.origin.y.0 * scale_factor) as f64,
        x1: ((window_bounds.origin.x.0 + window_bounds.size.width.0) * scale_factor) as f64,
        y1: ((window_bounds.origin.y.0 + window_bounds.size.height.0) * scale_factor) as f64,
    });
    for action in &node.actions {
        out.add_action(*action);
    }
    out
}

/// 在 [`Element::a11y_synthetic_children`] 中发布画布语义节点。
///
/// 每个 [`CanvasNode`] 会成为画布元素节点的一个合成子节点（带窗口坐标矩形，因此内置的
/// `Click` 回退也能命中它们）。若某个节点标记为
/// [`focused`][CanvasNode::focused]，同时会把它登记为父节点的 active descendant
/// （父元素自身可聚焦时，辅助技术据此播报当前对象）。
///
/// 返回已成功推入的节点标识（重复 id 会被框架丢弃）。
pub fn push_canvas_a11y(
    builder: &mut A11ySubtreeBuilder,
    semantics: &CanvasSemantics,
    canvas_origin: Point<Pixels>,
) -> Vec<CanvasNodeId> {
    let scale_factor = builder.scale_factor();
    let mut pushed = Vec::with_capacity(semantics.len());
    for node in semantics.nodes() {
        let bounds = window_bounds(canvas_origin, node);
        let node_id = builder.synthetic_node_id(node.id());
        if builder.push_child_with_bounds(
            node_id,
            accesskit_node(node, bounds, scale_factor),
            bounds,
        ) {
            if node.is_focused() {
                builder.parent_node().set_active_descendant(node_id);
            }
            pushed.push(node.id());
        }
    }
    pushed
}

/// 画布语义节点上的无障碍动作处理器。
pub type CanvasActionHandler = Rc<
    RefCell<dyn FnMut(CanvasNodeId, Action, Option<&ActionData>, &mut Window, &mut App) + 'static>,
>;

/// 在 [`Element::paint`] 中为画布语义节点登记动作监听器。
///
/// `canvas_id` 是画布元素自身的 [`GlobalElementId`]：合成子节点 id 由它派生，因此
/// 两侧（发布与登记）能对上。返回登记的监听器数量。
///
/// 仅在无障碍功能激活时调用（否则监听器表不会被框架清理）。
pub fn register_canvas_a11y_actions(
    window: &mut Window,
    canvas_id: &GlobalElementId,
    semantics: &CanvasSemantics,
    handler: CanvasActionHandler,
) -> usize {
    let parent = canvas_id.accesskit_node_id();
    let mut count = 0;
    for node in semantics.nodes() {
        for action in node.actions() {
            let handler = handler.clone();
            let id = node.id();
            let action = *action;
            window.on_a11y_action(
                synthetic_child_node_id(parent, id),
                action,
                move |data, window, cx| {
                    (handler.borrow_mut())(id, action, data, window, cx);
                },
            );
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    // 注：不能 `use super::*`——本模块的 glob 会把 `crate::test` 带进来遮蔽内置 `#[test]`。
    use super::{
        CanvasNode, CanvasSemantics, READING_ROW_TOLERANCE, accesskit_node, next_in_order,
        node_at_point, reading_order_indices, window_bounds,
    };
    use accesskit::{Action, Role};

    use crate::{Bounds, point, px, size};

    /// 造一个画布节点。
    fn node(id: u64, label: &str, x: f32, y: f32, w: f32, h: f32) -> CanvasNode {
        CanvasNode::new(
            id,
            Role::GraphicsObject,
            label,
            Bounds::new(point(px(x), px(y)), size(px(w), px(h))),
        )
    }

    /// 阅读顺序：先分行（y 容差内同行），行内按 x。
    #[test]
    fn reading_order_rows_then_columns() {
        // 第一行两个（y=0 / y=2 在同一容差内）、第二行两个（y=100）。
        let nodes = vec![
            node(1, "右一", 100.0, 0.0, 10.0, 10.0),
            node(2, "下一", 0.0, 100.0, 10.0, 10.0),
            node(3, "左一", 0.0, 2.0, 10.0, 10.0),
            node(4, "下二", 50.0, 100.0, 10.0, 10.0),
        ];
        let order = reading_order_indices(&nodes, READING_ROW_TOLERANCE);
        assert_eq!(order, vec![2, 0, 1, 3]);

        let mut semantics = CanvasSemantics::new();
        semantics.extend(nodes.clone());
        assert_eq!(semantics.reading_order(), vec![3, 1, 2, 4]);
        assert_eq!(semantics.first_in_reading_order(), Some(3));

        // 容差为 0 时 y 不同即分行。
        let order = reading_order_indices(&nodes, px(0.0));
        assert_eq!(order, vec![0, 2, 1, 3]);
    }

    /// 阅读顺序中前后移动：环绕，`None` 从首 / 末开始。
    #[test]
    fn reading_order_navigation_wraps() {
        let order = vec![0usize, 1, 2];
        assert_eq!(next_in_order(&order, None, true), Some(0));
        assert_eq!(next_in_order(&order, None, false), Some(2));
        assert_eq!(next_in_order(&order, Some(0), true), Some(1));
        assert_eq!(next_in_order(&order, Some(2), true), Some(0));
        assert_eq!(next_in_order(&order, Some(0), false), Some(2));
        assert_eq!(next_in_order(&[], None, true), None);

        // 状态接口：不在集合中的 current 也当作起步。
        let mut semantics = CanvasSemantics::new();
        semantics.push(node(7, "甲", 0.0, 0.0, 10.0, 10.0));
        semantics.push(node(8, "乙", 40.0, 0.0, 10.0, 10.0));
        assert_eq!(semantics.next_in_reading_order(None, true), Some(7));
        assert_eq!(semantics.next_in_reading_order(Some(7), true), Some(8));
        assert_eq!(semantics.next_in_reading_order(Some(8), true), Some(7));
        assert_eq!(semantics.next_in_reading_order(Some(8), false), Some(7));
        assert_eq!(semantics.next_in_reading_order(Some(999), false), Some(8));
    }

    /// 命中测试：后画者在上；矩形边界半开。
    #[test]
    fn hit_test_topmost_wins() {
        let nodes = vec![
            node(1, "底", 0.0, 0.0, 100.0, 100.0),
            node(2, "顶", 20.0, 20.0, 40.0, 40.0),
        ];
        assert_eq!(node_at_point(&nodes, point(px(30.0), px(30.0))), Some(1));
        assert_eq!(node_at_point(&nodes, point(px(90.0), px(90.0))), Some(0));
        // 右 / 下边界不含（半开区间）。
        assert_eq!(node_at_point(&nodes, point(px(100.0), px(100.0))), None);

        let mut semantics = CanvasSemantics::new();
        semantics.extend(nodes);
        assert_eq!(semantics.hit_test(point(px(30.0), px(30.0))), Some(2));
        assert_eq!(semantics.hit_test(point(px(500.0), px(500.0))), None);
        assert_eq!(semantics.hit_test(point(px(90.0), px(90.0))), Some(1));
    }

    /// 局部 → 窗口矩形换算与按 id 查询。
    #[test]
    fn window_bounds_translate_by_canvas_origin() {
        let mut semantics = CanvasSemantics::new();
        semantics.push(node(5, "对象", 10.0, 20.0, 30.0, 40.0));
        let origin = point(px(200.0), px(100.0));
        assert_eq!(
            semantics.window_bounds(origin, 5),
            Some(Bounds::new(
                point(px(210.0), px(120.0)),
                size(px(30.0), px(40.0))
            ))
        );
        assert_eq!(semantics.window_bounds(origin, 6), None);
        assert_eq!(
            window_bounds(origin, semantics.get(5).unwrap()),
            semantics.window_bounds(origin, 5).unwrap()
        );
    }

    /// AccessKit 节点映射：名称 / 值 / 说明 / 状态 / 动作 / 物理像素矩形。
    #[test]
    fn accesskit_node_maps_state_and_scaled_bounds() {
        let node = node(1, "单元格 B2", 10.0, 20.0, 30.0, 40.0)
            .with_value("42")
            .with_description("数值")
            .disabled(true)
            .selected(true)
            .with_actions([Action::Click, Action::Focus, Action::Click]);
        let bounds = Bounds::new(point(px(110.0), px(220.0)), size(px(30.0), px(40.0)));
        let out = accesskit_node(&node, bounds, 2.0);

        assert_eq!(out.role(), Role::GraphicsObject);
        assert_eq!(out.label(), Some("单元格 B2"));
        assert_eq!(out.value(), Some("42"));
        assert_eq!(out.description(), Some("数值"));
        assert!(out.is_disabled());
        assert_eq!(out.is_selected(), Some(true));
        // 支持的动作写入节点（位集，重复登记不出错）。
        assert!(out.supports_action(Action::Click));
        assert!(out.supports_action(Action::Focus));
        assert!(!out.supports_action(Action::Increment));
        let rect = out.bounds().expect("应写入矩形");
        assert_eq!(
            (rect.x0, rect.y0, rect.x1, rect.y1),
            (220.0, 440.0, 280.0, 520.0)
        );
    }

    /// 就地改状态与集合维护。
    #[test]
    fn semantics_mutation_helpers() {
        let mut semantics = CanvasSemantics::new();
        assert!(semantics.is_empty());
        semantics.push(node(1, "甲", 0.0, 0.0, 10.0, 10.0));
        semantics.push(node(2, "乙", 20.0, 0.0, 10.0, 10.0));
        assert_eq!(semantics.len(), 2);
        assert_eq!(semantics.index_of(2), Some(1));
        assert_eq!(semantics.iter().count(), 2);

        semantics.get_mut(2).unwrap().set_selected(true);
        assert!(semantics.get(2).unwrap().is_selected());
        assert!(!semantics.get(1).unwrap().is_selected());

        semantics.get_mut(1).unwrap().set_focused(true);
        assert!(semantics.get(1).unwrap().is_focused());

        semantics.clear();
        assert!(semantics.is_empty());
        assert_eq!(semantics.next_in_reading_order(None, true), None);
        // 容差设置不受 clear 影响。
        assert_eq!(semantics.row_tolerance(), READING_ROW_TOLERANCE);
        assert_eq!(
            CanvasSemantics::new()
                .with_row_tolerance(px(8.0))
                .row_tolerance(),
            px(8.0)
        );
    }
}
