//! 行内编辑：点击文本直接进入编辑态，支持回车保存 / Esc 取消。

use std::ops::Range;
use std::rc::Rc;

use crate::{prelude::FluentBuilder as _, *};

actions!(inline_edit, [Save, Cancel]);

/// 注册行内编辑的键盘绑定。
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Save, Some("InlineEdit")),
        KeyBinding::new("escape", Cancel, Some("InlineEdit")),
    ]);
}

/// 进入编辑态的触发方式。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum InlineEditTrigger {
    #[default]
    /// 单击进入编辑。
    Click,
    /// 双击进入编辑。
    DoubleClick,
}

/// 失焦时的行为。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum InlineEditBlurBehavior {
    #[default]
    /// 失焦保存。
    Save,
    /// 失焦取消。
    Cancel,
}

/// 行内编辑状态：管理文本、编辑态与选区。
pub struct InlineEditState {
    /// 已保存的值。
    value: String,
    /// 是否处于编辑态。
    editing: bool,
    /// 编辑中的临时值。
    edit_value: String,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 选中范围（UTF-8 字节）。
    selected_range: Range<usize>,
    /// 选区是否反向。
    selection_reversed: bool,
    /// 输入法标记范围。
    marked_range: Option<Range<usize>>,
    /// 上次排版的行。
    last_layout: Option<ShapedLine>,
    /// 上次布局边界。
    last_bounds: Option<Bounds<Pixels>>,
    /// 是否正在拖选。
    is_selecting: bool,
}

impl InlineEditState {
    /// 创建行内编辑状态。
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            value: String::new(),
            editing: false,
            edit_value: String::new(),
            focus_handle: cx.focus_handle(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    /// 以初始文本创建状态。
    pub fn with_value(cx: &mut Context<Self>, value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            value: value.clone(),
            editing: false,
            edit_value: value,
            focus_handle: cx.focus_handle(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    /// 获取已保存的值。
    pub fn value(&self) -> &str {
        &self.value
    }

    /// 设置值。
    pub fn set_value(&mut self, value: impl Into<String>, cx: &mut Context<Self>) {
        self.value = value.into();
        self.edit_value = self.value.clone();
        cx.notify();
    }

    /// 是否处于编辑态。
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// 进入编辑态，全选当前文本。
    pub fn start_editing(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            return;
        }
        self.editing = true;
        self.edit_value = self.value.clone();
        let len = self.edit_value.len();
        self.selected_range = 0..len;
        self.selection_reversed = false;
        cx.notify();
    }

    /// 保存编辑结果。
    pub fn save(&mut self, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.value = self.edit_value.clone();
        self.editing = false;
        self.selected_range = 0..0;
        cx.notify();
    }

    /// 取消编辑，丢弃临时值。
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.edit_value = self.value.clone();
        self.editing = false;
        self.selected_range = 0..0;
        cx.notify();
    }

    /// 获取编辑中的临时值。
    pub fn edit_value(&self) -> &str {
        &self.edit_value
    }

    /// 光标所在偏移（UTF-8 字节）。
    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// 将光标移到指定偏移。
    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify();
    }

    /// 扩展选区到指定偏移。
    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    /// 上一个字素边界。
    fn previous_boundary(&self, offset: usize) -> usize {
        use unicode_segmentation::UnicodeSegmentation;
        self.edit_value
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    /// 下一个字素边界。
    fn next_boundary(&self, offset: usize) -> usize {
        use unicode_segmentation::UnicodeSegmentation;
        self.edit_value
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.edit_value.len())
    }

    /// UTF-16 偏移转 UTF-8 偏移。
    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.edit_value.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    /// UTF-8 偏移转 UTF-16 偏移。
    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.edit_value.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    /// UTF-8 范围转 UTF-16 范围。
    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    /// UTF-16 范围转 UTF-8 范围。
    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    /// 根据鼠标位置计算文本偏移。
    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.edit_value.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.edit_value.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    /// 鼠标按下：开始拖选或移动光标。
    fn on_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        self.is_selecting = true;
        let click_index = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(click_index, cx);
        } else {
            self.move_to(click_index, cx);
        }
    }

    /// 鼠标松开：结束拖选。
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    /// 鼠标移动：拖选时扩展选区。
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    /// 保存动作。
    pub fn handle_save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        self.save(cx);
    }

    /// 取消动作。
    pub fn handle_cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel(cx);
    }

    /// 退格删除。
    pub fn handle_backspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    /// 删除键。
    pub fn handle_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    /// 光标左移。
    pub fn handle_left(&mut self, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    /// 光标右移。
    pub fn handle_right(&mut self, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    /// 全选。
    pub fn handle_select_all(&mut self, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.edit_value.len(), cx);
    }

    /// 移到开头。
    pub fn handle_home(&mut self, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    /// 移到末尾。
    pub fn handle_end(&mut self, cx: &mut Context<Self>) {
        self.move_to(self.edit_value.len(), cx);
    }

    /// 复制选中文本。
    pub fn handle_copy(&self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.edit_value[self.selected_range.clone()].to_string(),
            ));
        }
    }

    /// 剪切选中文本。
    pub fn handle_cut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.edit_value[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    /// 粘贴剪贴板文本（换行替换为空格）。
    pub fn handle_paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let filtered = text.replace('\n', " ");
            self.replace_text_in_range(None, &filtered, window, cx);
        }
    }
}

impl Focusable for InlineEditState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for InlineEditState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.edit_value[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.edit_value =
            self.edit_value[0..range.start].to_owned() + new_text + &self.edit_value[range.end..];
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.edit_value =
            self.edit_value[0..range.start].to_owned() + new_text + &self.edit_value[range.end..];
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

impl Render for InlineEditState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 行内编辑的文本绘制元素：负责选区、光标与输入法标记的渲染。
struct InlineEditTextElement {
    state: Entity<InlineEditState>,
    placeholder: SharedString,
}

/// 预绘制状态：排版行 + 光标 + 选区。
struct TextElementPrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for InlineEditTextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for InlineEditTextElement {
    type RequestLayoutState = ();
    type PrepaintState = TextElementPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self.state.read(cx);
        let content = &state.edit_value;
        let selected_range = state.selected_range.clone();
        let cursor = state.cursor_offset();
        let text_style = window.text_style();
        let theme = cx.theme();

        let (display_text, text_color) = if content.is_empty() {
            (
                self.placeholder.clone(),
                theme.tokens.muted_foreground.color,
            )
        } else {
            (SharedString::from(content.clone()), text_style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: text_style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let runs = if let Some(marked_range) = state.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len().saturating_sub(marked_range.end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let (selection, cursor_quad) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    theme.tokens.primary,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    theme.tokens.primary.opacity(0.2),
                )),
                None,
            )
        };

        TextElementPrepaintState {
            line: Some(line),
            cursor: cursor_quad,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.state.read(cx).focus_handle.clone();

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }

        let Some(line) = prepaint.line.take() else {
            return;
        };

        let _ = line.paint(
            bounds.origin,
            window.line_height(),
            TextAlign::default(),
            None,
            window,
            cx,
        );

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.state.update(cx, |state, _| {
            state.last_layout = Some(line);
            state.last_bounds = Some(bounds);
        });
    }

    #[cfg(feature = "dom-backend")]
    fn dom(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) -> Option<DomNode> {
        // 纯 DOM 模式下 canvas 不可见，行内编辑的可编辑文本必须进入 DOM 树，
        // 否则编辑态下文本不可见、且看起来「无法编辑」。复用与 canvas 一致的 display 计算。
        let state = self.state.read(cx);
        let text_style = window.text_style();
        let rem_size = window.rem_size();
        let theme = cx.theme();
        let focused = state.focus_handle.is_focused(window);

        let (display, color) = if state.edit_value.is_empty() {
            (
                self.placeholder.clone(),
                theme.tokens.muted_foreground.color,
            )
        } else {
            (
                SharedString::from(state.edit_value.clone()),
                text_style.color,
            )
        };

        if display.is_empty() {
            return None;
        }

        let font_size = text_style.font_size.to_pixels(rem_size);
        let line_height = text_style
            .line_height
            .to_pixels(text_style.font_size, rem_size);
        let run = TextRun {
            len: display.len(),
            font: text_style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(display.clone(), font_size, &[run], None);
        let cursor = state.cursor_offset();

        // 容器：可编辑文本本身（绝对定位，复用 bounds）。
        let mut container_style = DomStyle::from_bounds(bounds);
        container_style.color = Some(color);
        container_style.font_size = Some(font_size);
        container_style.font_family = Some(text_style.font_family.clone());
        container_style.font_weight = Some(text_style.font_weight);
        container_style.font_style = Some(text_style.font_style);
        container_style.line_height = Some(line_height);
        container_style.white_space = Some(text_style.white_space);

        // 文本子节点（沿用 from_bounds 绝对定位，作为容器首个子节点）。
        let mut text_style_dom = DomStyle::from_bounds(bounds);
        text_style_dom.color = Some(color);
        text_style_dom.font_size = Some(font_size);
        text_style_dom.font_family = Some(text_style.font_family.clone());
        text_style_dom.font_weight = Some(text_style.font_weight);
        text_style_dom.font_style = Some(text_style.font_style);
        text_style_dom.line_height = Some(line_height);
        text_style_dom.white_space = Some(text_style.white_space);

        let mut children = vec![DomNode {
            kind: DomNodeKind::Text { text: display },
            style: text_style_dom,
            scroll_handle: None,
        }];

        // 可见光标：聚焦时用一个绝对定位的细 div 模拟。
        if focused {
            let caret_x = line.x_for_index(cursor);
            let mut caret_style = DomStyle::from_bounds(Bounds {
                origin: point(bounds.origin.x + caret_x, bounds.origin.y),
                size: size(px(2.0), line_height),
            });
            caret_style.background_color = Some(theme.caret);
            children.push(DomNode {
                kind: DomNodeKind::Element {
                    tag: "div",
                    attrs: Vec::new(),
                    children: Vec::new(),
                },
                style: caret_style,
                scroll_handle: None,
            });
        }

        Some(DomNode {
            kind: DomNodeKind::Element {
                tag: "div",
                attrs: Vec::new(),
                children,
            },
            style: container_style,
            scroll_handle: None,
        })
    }
}

/// 行内编辑组件。
#[derive(IntoElement)]
pub struct InlineEdit {
    /// 绑定状态实体。
    state: Entity<InlineEditState>,
    /// 占位文本。
    placeholder: SharedString,
    /// 是否禁用。
    disabled: bool,
    /// 触发方式。
    trigger: InlineEditTrigger,
    /// 保存回调。
    on_save: Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>,
    /// 取消回调。
    on_cancel: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// 用户样式。
    style: StyleRefinement,
}

impl InlineEdit {
    /// 创建行内编辑，默认占位 "Click to edit"、单击触发。
    pub fn new(state: Entity<InlineEditState>) -> Self {
        Self {
            state,
            placeholder: "Click to edit".into(),
            disabled: false,
            trigger: InlineEditTrigger::Click,
            on_save: None,
            on_cancel: None,
            style: StyleRefinement::default(),
        }
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置是否禁用。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 设置触发方式。
    pub fn trigger(mut self, trigger: InlineEditTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    /// 设置保存回调。
    pub fn on_save<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_save = Some(Rc::new(handler));
        self
    }

    /// 设置取消回调。
    pub fn on_cancel<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_cancel = Some(Rc::new(handler));
        self
    }
}

impl Styled for InlineEdit {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for InlineEdit {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let user_style = self.style;
        let is_editing = self.state.read(cx).is_editing();
        let value = self.state.read(cx).value().to_string();
        let focus_handle = self.state.read(cx).focus_handle(cx);
        let state = self.state.clone();
        let state_for_click = self.state.clone();
        let state_for_double_click = self.state.clone();
        let state_for_mouse_down = self.state.clone();
        let state_for_mouse_up = self.state.clone();
        let state_for_mouse_move = self.state.clone();
        let on_save = self.on_save.clone();
        let on_cancel = self.on_cancel.clone();
        let trigger = self.trigger;
        let placeholder = self.placeholder.clone();

        if is_editing {
            // 聚焦外发光（替代旧库 focus_ring_light）。
            let focus_ring =
                BoxShadow::new(px(0.0), px(0.0), *theme.tokens.ring).blur_radius(px(6.0));

            let mut root = div()
                .id(("inline-edit", state.entity_id()))
                .key_context("InlineEdit")
                .track_focus(&focus_handle)
                .h(px(32.0))
                .px(px(8.0))
                .py(px(4.0))
                .bg(theme.tokens.background)
                .border_1()
                .border_color(theme.tokens.ring)
                .rounded(theme.radius)
                .shadow(vec![focus_ring])
                .text_size(px(14.0))
                .font_family(theme.font_family.clone())
                .text_color(theme.tokens.foreground)
                .on_action({
                    let state = state.clone();
                    let on_save = on_save.clone();
                    move |_: &Save, window: &mut Window, cx: &mut App| {
                        let value = state.read(cx).edit_value.clone();
                        state.update(cx, |s, cx| s.save(cx));
                        if let Some(handler) = on_save.as_ref() {
                            handler(&value, window, cx);
                        }
                    }
                })
                .on_action({
                    let state = state.clone();
                    let on_cancel = on_cancel.clone();
                    move |_: &Cancel, window: &mut Window, cx: &mut App| {
                        state.update(cx, |s, cx| s.cancel(cx));
                        if let Some(handler) = on_cancel.as_ref() {
                            handler(window, cx);
                        }
                    }
                })
                .on_key_down({
                    let state = state.clone();
                    move |event, window, cx| {
                        let key = event.keystroke.key.as_str();
                        match key {
                            "backspace" => {
                                state.update(cx, |s, cx| s.handle_backspace(window, cx));
                                cx.stop_propagation();
                            }
                            "delete" => {
                                state.update(cx, |s, cx| s.handle_delete(window, cx));
                                cx.stop_propagation();
                            }
                            "left" => {
                                state.update(cx, |s, cx| s.handle_left(cx));
                                cx.stop_propagation();
                            }
                            "right" => {
                                state.update(cx, |s, cx| s.handle_right(cx));
                                cx.stop_propagation();
                            }
                            "home" => {
                                state.update(cx, |s, cx| s.handle_home(cx));
                                cx.stop_propagation();
                            }
                            "end" => {
                                state.update(cx, |s, cx| s.handle_end(cx));
                                cx.stop_propagation();
                            }
                            "a" if event.keystroke.modifiers.platform => {
                                state.update(cx, |s, cx| s.handle_select_all(cx));
                                cx.stop_propagation();
                            }
                            "c" if event.keystroke.modifiers.platform => {
                                state.update(cx, |s, cx| s.handle_copy(cx));
                                cx.stop_propagation();
                            }
                            "x" if event.keystroke.modifiers.platform => {
                                state.update(cx, |s, cx| s.handle_cut(window, cx));
                                cx.stop_propagation();
                            }
                            "v" if event.keystroke.modifiers.platform => {
                                state.update(cx, |s, cx| s.handle_paste(window, cx));
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    }
                })
                .on_mouse_down(MouseButton::Left, {
                    move |event: &MouseDownEvent, _, cx| {
                        state_for_mouse_down.update(cx, |s, cx| s.on_mouse_down(event, cx));
                    }
                })
                .on_mouse_up(MouseButton::Left, {
                    move |event: &MouseUpEvent, _, cx| {
                        state_for_mouse_up.update(cx, |s, cx| s.on_mouse_up(event, cx));
                    }
                })
                .on_mouse_move({
                    move |event: &MouseMoveEvent, _, cx| {
                        state_for_mouse_move.update(cx, |s, cx| s.on_mouse_move(event, cx));
                    }
                })
                .child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .child(InlineEditTextElement {
                            state: state,
                            placeholder,
                        }),
                );

            root.style().refine(&user_style);
            root
        } else {
            let display_text = if value.is_empty() {
                self.placeholder.clone()
            } else {
                value.clone().into()
            };

            let text_color = if value.is_empty() {
                theme.tokens.muted_foreground
            } else {
                theme.tokens.foreground
            };

            let mut root = div()
                .id(("inline-edit-display", state.entity_id()))
                .h(px(32.0))
                .px(px(8.0))
                .py(px(4.0))
                .flex()
                .items_center()
                .text_size(px(14.0))
                .font_family(theme.font_family.clone())
                .text_color(text_color)
                .rounded(theme.radius)
                .when(!self.disabled, |d| {
                    d.cursor_pointer()
                        .hover(|s| {
                            s.bg(theme.tokens.muted.opacity(0.5))
                                .border_b_1()
                                .border_color(theme.tokens.primary.opacity(0.5))
                        })
                        .when(trigger == InlineEditTrigger::Click, |d| {
                            d.on_click({
                                move |_, window, cx| {
                                    state_for_click.update(cx, |s, cx| s.start_editing(cx));
                                    window.focus(&state_for_click.read(cx).focus_handle(cx), cx);
                                }
                            })
                        })
                        .when(trigger == InlineEditTrigger::DoubleClick, |d| {
                            d.on_mouse_down(MouseButton::Left, {
                                move |event: &MouseDownEvent, window, cx| {
                                    if event.click_count >= 2 {
                                        state_for_double_click
                                            .update(cx, |s, cx| s.start_editing(cx));
                                        window.focus(
                                            &state_for_double_click.read(cx).focus_handle(cx),
                                            cx,
                                        );
                                    }
                                }
                            })
                        })
                })
                .when(self.disabled, |d| d.opacity(0.5))
                .child(display_text);

            root.style().refine(&user_style);
            root
        }
    }
}
