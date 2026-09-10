//! 编辑器覆盖层渲染：折叠图标、多光标（`editor` feature 门控）。
//!
//! 从 `element.rs` 拆出，保持核心文本渲染与编辑器装饰分离。
//! 无 feature 时这些代码不编译，调用方（`element.rs`）对应调用点同步门控。

use crate::InteractiveElement;
use crate::prelude::FluentBuilder as _;
use crate::styled_ext::Selectable;
use crate::theme::ActiveTheme as _;
use crate::{
    AnyElement, App, Bounds, Button, ButtonVariants as _, Entity, Half, Hitbox, HitboxBehavior,
    IconName, IntoElement, MouseButton, ParentElement as _, Pixels, Point, Refineable, RenderOnce,
    Sizable as _, StatefulInteractiveElement, StyleRefinement, Styled, TextAlign, TextRun, Window,
    div, h_flex, point, px, size,
};

use super::super::blink_cursor::CURSOR_WIDTH;
use super::super::input::{FOLD_ICON_HITBOX_WIDTH, LINE_NUMBER_RIGHT_MARGIN};
use super::super::layout::LastLayout;
use super::super::rope_ext::RopeExt as _;
use super::super::{Input, InputState};
use super::inlay_hints::InlayHint;
use super::state::EditorState;
use crate::input_ui::InputContextMenuBuilder;
use crate::input_ui::{Enter, Escape, MoveDown, MoveUp};

/// 折叠图标本体宽度（布局与绘制共用）。
const FOLD_ICON_WIDTH: Pixels = px(14.);

/// 折叠图标布局信息。
pub(crate) struct FoldIconLayout {
    /// 行号区域命中框（用于悬停检测）
    line_number_hitbox: Hitbox,
    /// 每个折叠候选的 (display_row, is_folded, icon_element) 列表
    icons: Vec<(usize, bool, crate::AnyElement)>,
}

/// 布局折叠图标（第一遍收集候选，第二遍创建并预绘制图标）。
pub(crate) fn layout_fold_icons(
    state: &Entity<InputState>,
    origin_x: Pixels,
    bounds: &Bounds<Pixels>,
    last_layout: &LastLayout,
    window: &mut Window,
    cx: &mut App,
) -> FoldIconLayout {
    // 第一遍：从状态收集折叠信息
    struct FoldInfo {
        buffer_line: usize,
        is_folded: bool,
        display_row: usize,
        offset_y: Pixels,
    }

    let line_number_hitbox = window.insert_hitbox(
        Bounds::new(
            point(origin_x, bounds.origin.y + last_layout.visible_top),
            size(last_layout.line_number_width, bounds.size.height),
        ),
        HitboxBehavior::Normal,
    );

    let mut icon_layout = FoldIconLayout {
        line_number_hitbox,
        icons: vec![],
    };

    let fold_infos: Vec<FoldInfo> = {
        let state = state.read(cx);
        if !state.mode.is_folding() {
            return icon_layout;
        }

        let mut infos = Vec::with_capacity(last_layout.visible_buffer_lines.len());
        let mut offset_y = last_layout.visible_top;

        for (line, &buffer_line) in last_layout
            .lines
            .iter()
            .zip(last_layout.visible_buffer_lines.iter())
        {
            if state.display_map.is_fold_candidate(buffer_line) {
                let is_folded = state.display_map.is_folded_at(buffer_line);
                infos.push(FoldInfo {
                    buffer_line,
                    is_folded,
                    display_row: buffer_line,
                    offset_y,
                });
            }

            offset_y += line.wrapped_lines.len() * last_layout.line_height;
        }

        infos
    }; // state 在此处释放

    // 第二遍：创建并预绘制图标
    let line_height = last_layout.line_height;
    let line_number_width =
        last_layout.line_number_width - LINE_NUMBER_RIGHT_MARGIN - FOLD_ICON_HITBOX_WIDTH;
    let icon_relative_pos = point(
        (FOLD_ICON_HITBOX_WIDTH - FOLD_ICON_WIDTH).half(),
        (line_height - FOLD_ICON_WIDTH).half(),
    );

    for (ix, info) in fold_infos.iter().enumerate() {
        // 将折叠图标放在行号右侧。
        // 使用 origin_x（未滚动）使图标在水平滚动时保持在沟槽中。
        let fold_icon_bounds = Bounds::new(
            point(
                origin_x + icon_relative_pos.x + line_number_width,
                bounds.origin.y + icon_relative_pos.y + info.offset_y,
            ),
            size(FOLD_ICON_HITBOX_WIDTH, line_height),
        );

        // 创建并预绘制图标
        let mut icon = Button::new(("fold", ix))
            .ghost()
            .icon(if info.is_folded {
                IconName::ChevronRight
            } else {
                IconName::ChevronDown
            })
            .xsmall()
            .rounded_xs()
            .size(FOLD_ICON_WIDTH)
            .selected(info.is_folded)
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                let buffer_line = info.buffer_line;
                move |_, _: &mut Window, cx: &mut App| {
                    cx.stop_propagation();

                    state.update(cx, |state, cx| {
                        state.display_map.toggle_fold(buffer_line);
                        cx.notify();
                    });
                }
            })
            .into_any_element();

        icon.prepaint_as_root(
            fold_icon_bounds.origin,
            fold_icon_bounds.size.into(),
            window,
            cx,
        );

        icon_layout
            .icons
            .push((info.display_row, info.is_folded, icon));
    }

    icon_layout
}

/// 绘制折叠图标（仅悬停或当前行可见）。
pub(crate) fn paint_fold_icons(
    fold_icon_layout: &mut FoldIconLayout,
    current_row: Option<usize>,
    window: &mut Window,
    cx: &mut App,
) {
    let is_hovered = fold_icon_layout.line_number_hitbox.is_hovered(window);
    for (display_row, is_folded, icon) in fold_icon_layout.icons.iter_mut() {
        let is_current_line = current_row == Some(*display_row);

        if !is_hovered && !is_current_line && !*is_folded {
            continue;
        }

        icon.paint(window, cx);
    }
}

/// 计算额外光标边界（多光标，与主光标同尺寸同滚动，不参与滚动驱动）。
pub(crate) fn extra_cursor_bounds(
    state: &InputState,
    caret_for: &dyn Fn(usize, usize, bool) -> Point<Pixels>,
    bounds: &Bounds<Pixels>,
    line_number_width: Pixels,
    cursor_scroll_x: Pixels,
    line_height: Pixels,
    cursor_height: Pixels,
    text_align: TextAlign,
) -> Vec<Bounds<Pixels>> {
    let mut extra_cursor_bounds = Vec::new();
    if !state.masked {
        for extra in &state.extra_selections {
            let end = extra.end.min(state.core.text.len());
            let row = state.core.text.offset_to_point(end).row;
            let pos = caret_for(row, end, false);
            let x = bounds.left() + pos.x + line_number_width + cursor_scroll_x;
            let x = if text_align == TextAlign::Right {
                x.min(bounds.right() - CURSOR_WIDTH)
            } else {
                x
            };
            extra_cursor_bounds.push(Bounds::new(
                point(
                    x,
                    bounds.top() + pos.y + ((line_height - cursor_height) / 2.),
                ),
                size(CURSOR_WIDTH, cursor_height),
            ));
        }
    }
    extra_cursor_bounds
}

/// 行内提示绘制（paint 阶段 overlay；`editor` feature 门控）。
///
/// 文本流之外：不占布局（换行/滚动尺寸无感）、不进 `Rope`、不碰选区与命中测试
/// （三不）；只画可见行（不可见/折叠行跳过）；hint 文本逐帧塑形（hint 少，
/// provider 限可见范围，M4 约束）；多行文本只取首行（`shape_line` 遇换行
/// panic，见其文档）；越界偏移钳制到文本末尾。
pub(crate) fn paint_inlay_hints(
    state: &Entity<InputState>,
    last_layout: &LastLayout,
    text_origin: Point<Pixels>,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    let (enabled, hints, text) = state.read_with(cx, |state, _| {
        (
            state.inlay_hints_enabled,
            state.inlay_hints.clone(),
            state.text().clone(),
        )
    });
    if !enabled || hints.is_empty() {
        return;
    }
    // 按 buffer 行分组（越界钳制）。
    let mut by_row: std::collections::HashMap<usize, Vec<&InlayHint>> =
        std::collections::HashMap::new();
    for hint in &hints {
        let offset = hint.offset.min(text.len());
        let row = text.offset_to_point(offset).row;
        by_row.entry(row).or_default().push(hint);
    }
    let style = window.text_style();
    let font_size = style.font_size.to_pixels(window.rem_size());
    let color = cx.theme().muted_foreground;
    // 与文本绘制同循环（行高累加口径一致，见 element paint）。
    let mut y = px(0.);
    for ((line_layout, &buffer_row), &row_start) in last_layout
        .lines
        .iter()
        .zip(last_layout.visible_buffer_lines.iter())
        .zip(last_layout.visible_line_byte_offsets.iter())
    {
        if let Some(row_hints) = by_row.get(&buffer_row) {
            for hint in row_hints {
                let offset = hint.offset.min(text.len());
                let local = offset.saturating_sub(row_start);
                let Some(pos) = line_layout.position_for_index(local, last_layout, true) else {
                    continue;
                };
                let first_line = hint.text.lines().next().unwrap_or("");
                if first_line.is_empty() {
                    continue;
                }
                let shaped = window.text_system().shape_line(
                    first_line.to_string().into(),
                    font_size,
                    &[TextRun {
                        len: first_line.len(),
                        font: style.font(),
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                let _ = shaped.paint(
                    point(text_origin.x + pos.x, text_origin.y + y + pos.y),
                    line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }
        y += line_layout.size(line_height).height;
    }
}

/// 代码编辑器组件：粘性大纲顶栏 + 内部 `Input` 全尺寸 + 状态行（行号/列号）。
///
/// 与表单 `Input` 的区别：`Editor` 消费 `EditorState`（大纲/符号跳转/高亮透传
/// 内聚在状态里）；编辑器行为（行号/折叠/键入体验）由 `EditorState::new` 一次配好。
/// 顶栏只在开关开且大纲栈非空时出现（固定一行，`flex_none`，不挤占编辑区滚动）；
/// 点击面包屑走 `goto_symbol`。
#[derive(IntoElement)]
pub struct Editor {
    editor: Entity<EditorState>,
    /// 右键追加项（`Input::context_menu_extra` 透传；默认保留，跟分隔符后）。
    context_menu_extra: Option<InputContextMenuBuilder>,
    /// 右键完全接管（`Input::context_menu_override` 透传；默认不要）。
    context_menu_override: Option<InputContextMenuBuilder>,
    /// 右键总开关（`None` = 默认启用）。
    show_context_menu: Option<bool>,
    style: StyleRefinement,
}

impl Editor {
    /// 由编辑器状态创建（`cx.new(|cx| EditorState::new(...))` 的实体）。
    pub fn new(editor: &Entity<EditorState>) -> Self {
        Self {
            editor: editor.clone(),
            context_menu_extra: None,
            context_menu_override: None,
            show_context_menu: None,
            style: StyleRefinement::default(),
        }
    }

    /// 右键追加自定义项（透传内部 `Input`；默认菜单保留）。
    pub fn context_menu_extra(
        mut self,
        builder: impl Fn(
            crate::menu::PopupMenu,
            Entity<InputState>,
            &mut Window,
            &mut App,
        ) -> crate::menu::PopupMenu
        + 'static,
    ) -> Self {
        self.context_menu_extra = Some(std::rc::Rc::new(builder));
        self
    }

    /// 右键完全接管（透传内部 `Input`；默认菜单不要，可调默认 builder 拼回）。
    pub fn context_menu_override(
        mut self,
        builder: impl Fn(
            crate::menu::PopupMenu,
            Entity<InputState>,
            &mut Window,
            &mut App,
        ) -> crate::menu::PopupMenu
        + 'static,
    ) -> Self {
        self.context_menu_override = Some(std::rc::Rc::new(builder));
        self
    }

    /// 右键总开关（透传内部 `Input`）。
    pub fn show_context_menu(mut self, show: bool) -> Self {
        self.show_context_menu = Some(show);
        self
    }
}

impl Styled for Editor {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// 大纲栈面包屑元素（顶栏/状态行共用；点击跳转符号头）。
fn crumb_elements(
    editor: &Entity<EditorState>,
    stack: Vec<crate::highlight::DocumentSymbol>,
    id_prefix: &'static str,
) -> Vec<AnyElement> {
    let mut crumbs: Vec<AnyElement> = Vec::new();
    let last = stack.len().saturating_sub(1);
    for (ix, symbol) in stack.into_iter().enumerate() {
        let editor = editor.clone();
        crumbs.push(
            div()
                .id((id_prefix, ix))
                .cursor_pointer()
                .child(symbol.name.clone())
                .on_click(move |_, _, cx| {
                    editor.update(cx, |state, cx| {
                        state.goto_symbol(&symbol, cx);
                    });
                })
                .into_any_element(),
        );
        if ix != last {
            crumbs.push(div().child("›").into_any_element());
        }
    }
    crumbs
}

impl RenderOnce for Editor {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // 状态行：Ln 行， Col 列（字节列，CJK 以字节计，演示够用）。
        let (input, stack, sticky_position) = self.editor.read_with(cx, |state, cx| {
            let stack = if state.sticky_scroll_enabled() {
                state.sticky_stack(cx)
            } else {
                Vec::new()
            };
            (state.input().clone(), stack, state.sticky_position())
        });
        let (row, col) = input.read_with(cx, |state, _| {
            let cursor = state.cursor();
            let text = state.text();
            let row = text.offset_to_point(cursor).row;
            let col = cursor - text.line_start_offset(row);
            (row + 1, col + 1)
        });
        // Vim 模式指示（未启用为空，状态行不展示）。
        let vim_indicator = self.editor.read_with(cx, |state, cx| {
            state
                .vim_mode(cx)
                .map(|mode| format!(" · {}", mode.indicator()))
                .unwrap_or_default()
        });
        // 粘性顶栏：面包屑（点击跳转符号头；`Status` 位置时顶栏永不出现，
        // 面包屑改由状态行渲染，空栈非空栈高度一致，编辑区不跳动）。
        let editor = self.editor.clone();
        let show_top_bar = matches!(sticky_position, super::sticky_scroll::StickyPosition::Top);
        let muted = cx.theme().muted_foreground;
        let (sticky, status_crumbs): (Option<AnyElement>, Vec<AnyElement>) = if show_top_bar {
            let bar = (!stack.is_empty()).then(|| {
                div()
                    .flex_none()
                    .px(px(12.0))
                    .py(px(2.0))
                    .text_xs()
                    .text_color(muted)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .children(crumb_elements(&editor, stack, "sticky-crumb"))
                    .into_any_element()
            });
            (bar, Vec::new())
        } else {
            (None, crumb_elements(&editor, stack, "status-crumb"))
        };
        let user_style = self.style;
        // 内部 Input 组装（三档右键透传；闭包包一层 Rc 转发）。
        let mut input_el = Input::new(&input).flex_1();
        if let Some(show) = self.show_context_menu {
            input_el = input_el.show_context_menu(show);
        }
        if let Some(extra) = self.context_menu_extra {
            input_el = input_el
                .context_menu_extra(move |menu, state, window, cx| extra(menu, state, window, cx));
        }
        if let Some(override_builder) = self.context_menu_override {
            input_el = input_el.context_menu_override(move |menu, state, window, cx| {
                override_builder(menu, state, window, cx)
            });
        }
        // 缩略图浮层（相对定位盖右侧；关闭时不建元素，零开销）。
        let minimap = self
            .editor
            .read_with(cx, |state, _| state.minimap_enabled());
        let editor_for_minimap = self.editor.clone();
        // 补全菜单键盘接管（捕获阶段先于内部 `Input`，菜单收起时原样放行）。
        //
        // 焦点仍在编辑器内，无需把焦点移到候选框：菜单激活时 Up/Down 改选、
        // 主回车确认、Esc 收起，并 `stop_propagation` 吞掉 `Input` 的默认行为
        //（光标移动/换行/清空选区）；`Shift`/`secondary` 回车保持换行语义。
        let editor_for_keys = self.editor.clone();
        crate::v_flex()
            .size_full()
            .children(sticky)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .child(input_el)
                    .when(minimap, |this| {
                        this.child(super::minimap::render_minimap(&editor_for_minimap, cx))
                    }),
            )
            .child(
                crate::div()
                    .flex_none()
                    .px(px(12.0))
                    .py(px(4.0))
                    .text_xs()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(format!("Ln {row}, Col {col}{vim_indicator}"))
                    .children((!status_crumbs.is_empty()).then(|| {
                        // 状态栏面包屑（`Status` 位置；与行列号同行，高度恒定）。
                        h_flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(div().text_color(muted).child("·"))
                            .children(status_crumbs)
                            .into_any_element()
                    })),
            )
            .capture_action({
                let editor = editor_for_keys.clone();
                move |_: &MoveUp, _: &mut Window, cx: &mut App| {
                    if editor.read(cx).completion_menu_active() {
                        editor.update(cx, |state, cx| state.select_previous_completion(cx));
                        cx.stop_propagation();
                    }
                }
            })
            .capture_action({
                let editor = editor_for_keys.clone();
                move |_: &MoveDown, _: &mut Window, cx: &mut App| {
                    if editor.read(cx).completion_menu_active() {
                        editor.update(cx, |state, cx| state.select_next_completion(cx));
                        cx.stop_propagation();
                    }
                }
            })
            .capture_action({
                let editor = editor_for_keys.clone();
                move |action: &Enter, window: &mut Window, cx: &mut App| {
                    if !action.secondary
                        && !action.shift
                        && editor.read(cx).completion_menu_active()
                    {
                        editor.update(cx, |state, cx| {
                            state.accept_completion(None, window, cx);
                        });
                        cx.stop_propagation();
                    }
                }
            })
            .capture_action({
                let editor = editor_for_keys;
                move |_: &Escape, _: &mut Window, cx: &mut App| {
                    if editor.read(cx).completion_menu_active() {
                        editor.update(cx, |state, cx| state.dismiss_completion(cx));
                        cx.stop_propagation();
                    }
                }
            })
            // Vim 按键分发（O3；绑定命中即激活态，无条件吞传播）。
            .capture_action({
                let editor = self.editor.clone();
                move |action: &super::vim::VimKey, window: &mut Window, cx: &mut App| {
                    editor.update(cx, |state, cx| {
                        state.vim_key(action.key.as_ref(), window, cx);
                    });
                    cx.stop_propagation();
                }
            })
            // Vim Esc 模式切换（`Input` 自身 Esc 照跑只做折叠，不吞）。
            .capture_key_down({
                let editor = self.editor;
                move |event: &crate::KeyDownEvent, window: &mut Window, cx: &mut App| {
                    if event.keystroke.key.as_str() == "escape" {
                        editor.update(cx, |state, cx| {
                            state.vim_escape(window, cx);
                        });
                    }
                }
            })
            .map(|mut this| {
                this.style().refine(&user_style);
                this
            })
    }
}
