//! Markdown 文档级选区：命中测试、高亮与复制文本。
//!
//! 渲染时每个可选中的文本叶子会向 [`MarkdownSelectionState`] 登记自身纯文本
//! （列表项带 `• ` / `1. ` 前缀），选中状态用「叶子索引 + 块内字节偏移」表示。
//! 这样跨段落/跨列表/跨代码块拖选只需要在相邻叶子之间推进 head，复制时再按
//! 叶子顺序拼接文本。

use rgpui::{
    App, Bounds, Context, CursorStyle, DispatchPhase, Element, ElementId, Entity, GlobalElementId,
    Hitbox, HitboxBehavior, InspectorElementId, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, SharedString, StyledText, Window, accesskit,
};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

/// 选区端点：叶子索引 + 叶子内 UTF-8 字节偏移。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SelectionPoint {
    /// 文本叶子在文档中的顺序索引。
    pub leaf: usize,
    /// 叶子内字节偏移（字符边界）。
    pub byte: usize,
}

impl SelectionPoint {
    /// 创建端点。
    pub fn new(leaf: usize, byte: usize) -> Self {
        Self { leaf, byte }
    }
}

/// 一段文档选区。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownSelection {
    /// 拖选起点。
    pub anchor: SelectionPoint,
    /// 拖选终点（当前鼠标位置）。
    pub head: SelectionPoint,
}

impl MarkdownSelection {
    /// 按文档顺序返回 `(起点, 终点)`。
    pub fn ordered(&self) -> (SelectionPoint, SelectionPoint) {
        if (self.anchor.leaf, self.anchor.byte) <= (self.head.leaf, self.head.byte) {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// 是否为空选区（只是光标位置）。
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }

    /// 叶子 `leaf` 被选中的字节区间；未选中返回 `None`。
    ///
    /// 中间整叶返回整段，首尾叶按端点裁剪。端点落在字符中间时向下吸附到
    /// 字符边界，避免后续切片 panic。
    pub fn range_for_leaf(&self, leaf: usize, text: &str) -> Option<Range<usize>> {
        if self.is_collapsed() {
            return None;
        }
        let (start, end) = self.ordered();
        if leaf < start.leaf || leaf > end.leaf {
            return None;
        }
        let len = text.len();
        let from = if leaf == start.leaf {
            floor_char_boundary(text, start.byte)
        } else {
            0
        };
        let to = if leaf == end.leaf {
            floor_char_boundary(text, end.byte)
        } else {
            len
        };
        // 同一叶上端点重合时 `from == to`，视为未选中。
        (from < to).then_some(from..to)
    }

    /// 按顺序拼接选中文本；叶子之间用换行分隔，列表项保留标记前缀。
    pub(crate) fn selected_text(&self, leaves: &[LeafEntry]) -> String {
        if self.is_collapsed() {
            return String::new();
        }
        let (start, end) = self.ordered();
        let mut out = String::new();
        for (index, entry) in leaves.iter().enumerate() {
            if index < start.leaf {
                continue;
            }
            if index > end.leaf {
                break;
            }
            let Some(range) = self.range_for_leaf(index, &entry.text) else {
                continue;
            };
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&entry.prefix);
            out.push_str(&entry.text[range]);
        }
        out
    }
}

/// 渲染时登记的文本叶子。
#[derive(Clone, Debug)]
pub(crate) struct LeafEntry {
    /// 复制时加在文本前的标记（列表项用，普通段落为空）。
    pub prefix: String,
    /// 叶子展示的纯文本。
    pub text: SharedString,
}

impl LeafEntry {
    /// 创建叶子条目。
    pub(crate) fn new(prefix: impl Into<String>, text: impl Into<SharedString>) -> Self {
        Self {
            prefix: prefix.into(),
            text: text.into(),
        }
    }
}

/// Markdown 选区状态：由宿主创建为 `Entity`，传给 [`Markdown::selection`]。
pub struct MarkdownSelectionState {
    selection: Option<MarkdownSelection>,
    /// 鼠标按下且尚未抬起（拖选进行中）。
    pressed: bool,
    /// 最近一次渲染登记的叶子（渲染时重建，复制时读取）。
    leaves: Rc<RefCell<Vec<LeafEntry>>>,
}

impl Default for MarkdownSelectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownSelectionState {
    /// 创建空选区状态。
    pub fn new() -> Self {
        Self {
            selection: None,
            pressed: false,
            leaves: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// 当前选区。
    pub fn selection(&self) -> Option<MarkdownSelection> {
        self.selection
    }

    /// 是否正在拖选（鼠标按下后尚未抬起）。
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// 开始选择：记录锚点并进入按下状态。
    pub fn begin(&mut self, leaf: usize, byte: usize, cx: &mut Context<Self>) {
        let point = SelectionPoint::new(leaf, byte);
        self.selection = Some(MarkdownSelection {
            anchor: point,
            head: point,
        });
        self.pressed = true;
        cx.notify();
    }

    /// 拖选推进 head。
    pub fn extend(&mut self, leaf: usize, byte: usize, cx: &mut Context<Self>) {
        if !self.pressed {
            return;
        }
        let Some(selection) = self.selection.as_mut() else {
            return;
        };
        let point = SelectionPoint::new(leaf, byte);
        if selection.head != point {
            selection.head = point;
            cx.notify();
        }
    }

    /// 结束拖选（鼠标抬起）。
    pub fn end(&mut self, cx: &mut Context<Self>) {
        if self.pressed {
            self.pressed = false;
            cx.notify();
        }
    }

    /// 清空选区。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if self.selection.take().is_some() || self.pressed {
            self.pressed = false;
            cx.notify();
        }
    }

    /// 全选当前文档（按最近一次渲染登记的叶子）。
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let (first, last) = {
            let leaves = self.leaves.borrow();
            let Some(last) = leaves.len().checked_sub(1) else {
                return;
            };
            let last_len = leaves[last].text.len();
            (
                SelectionPoint::new(0, 0),
                SelectionPoint::new(last, last_len),
            )
        };
        self.selection = Some(MarkdownSelection {
            anchor: first,
            head: last,
        });
        self.pressed = false;
        cx.notify();
    }

    /// 选中文本；空选区返回 `None`。
    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let text = selection.selected_text(&self.leaves.borrow());
        (!text.is_empty()).then_some(text)
    }

    /// 复制选中的文本到系统剪贴板。
    pub fn copy_to_clipboard(&self, cx: &mut App) -> bool {
        let Some(text) = self.selected_text() else {
            return false;
        };
        cx.write_to_clipboard(rgpui::ClipboardItem::new_string(text));
        true
    }

    /// 渲染入口取叶子登记表（每帧先清空再顺序登记）。
    pub(crate) fn leaves_handle(&self) -> Rc<RefCell<Vec<LeafEntry>>> {
        self.leaves.clone()
    }
}

/// 向下吸附到字符边界；越界返回文本长度。
fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// 渲染期的选区上下文：登记叶子、计算高亮区间。
///
/// 克隆只复制实体句柄、`Rc` 登记表与选区快照，递归渲染时按需克隆即可。
#[derive(Clone)]
pub(crate) struct SelectionRender {
    /// 选区状态实体。
    pub(crate) state: Entity<MarkdownSelectionState>,
    /// 本帧叶子登记表。
    pub(crate) leaves: Rc<RefCell<Vec<LeafEntry>>>,
    /// 本帧快照的选区（渲染期间不变）。
    pub(crate) selection: Option<MarkdownSelection>,
    /// 高亮背景色。
    pub(crate) highlight: rgpui::Hsla,
}

impl SelectionRender {
    /// 开始一帧渲染：清空叶子登记表并快照当前选区。
    pub(crate) fn new(
        state: &Entity<MarkdownSelectionState>,
        highlight: rgpui::Hsla,
        cx: &rgpui::App,
    ) -> Self {
        let state_ref = state.read(cx);
        let leaves = state_ref.leaves_handle();
        leaves.borrow_mut().clear();
        Self {
            state: state.clone(),
            leaves,
            selection: state_ref.selection(),
            highlight,
        }
    }

    /// 登记一个文本叶子并返回索引。
    pub(crate) fn register(&mut self, prefix: &str, text: &str) -> usize {
        let mut leaves = self.leaves.borrow_mut();
        let index = leaves.len();
        leaves.push(LeafEntry::new(prefix, text));
        index
    }

    /// 叶子被选中的字节区间。
    pub(crate) fn range_for(&self, leaf: usize, text: &str) -> Option<Range<usize>> {
        self.selection?.range_for_leaf(leaf, text)
    }
}

/// 可拖选的文本元素（选区模式的叶子渲染器）。
pub(crate) struct SelectableText {
    element_id: ElementId,
    text: StyledText,
    plain: SharedString,
    leaf: usize,
    state: Entity<MarkdownSelectionState>,
    links: Vec<(Range<usize>, String)>,
    on_link: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
}

impl SelectableText {
    /// 创建可拖选文本。
    pub(crate) fn new(
        element_id: ElementId,
        text: StyledText,
        plain: SharedString,
        leaf: usize,
        state: Entity<MarkdownSelectionState>,
        links: Vec<(Range<usize>, String)>,
        on_link: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
    ) -> Self {
        Self {
            element_id,
            text,
            plain,
            leaf,
            state,
            links,
            on_link,
        }
    }
}

impl Element for SelectableText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        Some(accesskit::Role::Label)
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_value(self.plain.to_string());
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        self.text
            .prepaint(None, inspector_id, bounds, state, window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.text.layout().clone();
        self.text
            .paint(None, inspector_id, bounds, state, &mut (), window, cx);

        // 光标样式只能在 paint 阶段设置：悬停在文本上显示 I 形光标。
        if hitbox.is_hovered(window) {
            window.set_cursor_style(CursorStyle::IBeam, hitbox);
        }

        let leaf = self.leaf;
        let state = self.state.clone();

        // 按下：把布局命中的字节索引记为锚点。
        {
            let layout = layout.clone();
            let hitbox = hitbox.clone();
            let state = state.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Left
                    && hitbox.is_hovered(window)
                    && let Some(index) = index_for(&layout, event.position)
                {
                    state.update(cx, |state, cx| state.begin(leaf, index, cx));
                }
            });
        }

        // 移动：拖选中推进 head。
        {
            let layout = layout.clone();
            let hitbox = hitbox.clone();
            let state = state.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || !hitbox.is_hovered(window) {
                    return;
                }
                if state.read(cx).is_pressed()
                    && let Some(index) = index_for(&layout, event.position)
                {
                    state.update(cx, |state, cx| state.extend(leaf, index, cx));
                }
            });
        }

        // 抬起：结束拖选；未拖动的单击落在链接上时打开链接。
        {
            let hitbox = hitbox.clone();
            let links = self.links.clone();
            let on_link = self.on_link.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                    return;
                }
                let was_pressed = state.read(cx).is_pressed();
                let selection = state.read(cx).selection();
                state.update(cx, |state, cx| state.end(cx));
                if !was_pressed || !hitbox.is_hovered(window) {
                    return;
                }
                let Some(selection) = selection else {
                    return;
                };
                if !selection.is_collapsed() {
                    return;
                }
                let Some(index) = index_for(&layout, event.position) else {
                    return;
                };
                if selection.anchor.leaf != leaf || selection.anchor.byte != index {
                    return;
                }
                let Some((_, url)) = links.iter().find(|(range, _)| range.contains(&index)) else {
                    return;
                };
                if let Some(on_link) = on_link.as_ref() {
                    on_link(url, window, cx);
                }
            });
        }
    }
}

impl rgpui::IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// 布局命中：`Ok`/`Err` 都带最近字节索引。
fn index_for(layout: &rgpui::TextLayout, position: rgpui::Point<Pixels>) -> Option<usize> {
    match layout.index_for_position(position) {
        Ok(index) | Err(index) => Some(index),
    }
}

#[cfg(test)]
mod tests {
    use super::{LeafEntry, MarkdownSelection, SelectionPoint};

    fn leaves() -> Vec<LeafEntry> {
        vec![
            LeafEntry::new("", "第一段"),
            LeafEntry::new("• ", "条目"),
            LeafEntry::new("", "code()"),
        ]
    }

    /// 跨叶选区按顺序拼接，列表项保留前缀。
    #[test]
    fn selected_text_spans_leaves_in_document_order() {
        let selection = MarkdownSelection {
            anchor: SelectionPoint::new(2, 4),
            head: SelectionPoint::new(0, 3),
        };
        assert_eq!(selection.selected_text(&leaves()), "一段\n• 条目\ncode");
    }

    /// 中间整叶全部选中，端点叶按字节裁剪；空选区没有文本。
    #[test]
    fn range_clamps_at_leaf_boundaries() {
        let selection = MarkdownSelection {
            anchor: SelectionPoint::new(0, 3),
            head: SelectionPoint::new(2, 4),
        };
        assert_eq!(selection.range_for_leaf(0, "第一段"), Some(3..9));
        assert_eq!(selection.range_for_leaf(1, "• 条目"), Some(0..10));
        assert_eq!(selection.range_for_leaf(2, "code()"), Some(0..4));
        assert_eq!(selection.range_for_leaf(3, "code()"), None);

        let collapsed = MarkdownSelection {
            anchor: SelectionPoint::new(1, 2),
            head: SelectionPoint::new(1, 2),
        };
        assert_eq!(collapsed.range_for_leaf(1, "• 条目"), None);
        assert_eq!(collapsed.selected_text(&leaves()), "");
    }

    /// 端点落在多字节字符内部时向下吸附，切片不 panic。
    #[test]
    fn range_snaps_to_char_boundaries() {
        let selection = MarkdownSelection {
            anchor: SelectionPoint::new(0, 1),
            head: SelectionPoint::new(0, 4),
        };
        // “第”占 3 字节：1 吸附到 0，4 吸附到 3。
        assert_eq!(selection.range_for_leaf(0, "第一段"), Some(0..3));
        assert_eq!(selection.selected_text(&leaves()), "第");
    }
}
