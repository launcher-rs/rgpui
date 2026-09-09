//! 文本输入状态，保存 [`super::Input`] 的编辑状态。
//!
//! 从 rgpui-component 移植，裁剪了 LSP 集成、语法高亮、搜索面板、
//! 弹窗（诊断/悬停/上下文菜单）以及内联补全等非核心功能。

/// 高亮接线类型（`editor` feature 门控；默认构建不用）。
#[cfg(feature = "editor")]
use crate::highlight::{Highlighter, ThemeHighlightResolver};
use crate::menu::ContextMenuExt as _;
use crate::menu::{SelectDown, SelectLeft, SelectRight, SelectUp};
use crate::sum_tree::Bias;
use crate::{
    Action, App, AppContext, Bounds, ClipboardItem, Context, Edges, ElementSize, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, Pixels, Point, Render, ScrollHandle, ScrollWheelEvent, SharedString,
    Styled as _, Subscription, TextAlign, UTF16Selection, Window, div, point,
    prelude::FluentBuilder as _, px,
};
// `ruler_color` 字段类型用（`editor` feature 门控，默认构建不用）。
#[cfg(feature = "editor")]
use crate::Hsla;
use regex::Regex;
use ropey::{Rope, RopeSlice};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;
use unicode_segmentation::*;

use super::super::{
    DisplayMap, LastLayout, MASK_CHAR, Position, RopeExt as _, Selection, WrappingIndent,
    auto_scroll::AutoScroll,
    blink_cursor::{BlinkCursor, CURSOR_WIDTH},
    context_menu::InputContextMenuBuilder,
    core::TextCore,
    movement::MoveDirection,
};
/// 编辑器接线类型（`editor` feature 门控；默认构建不用）。
#[cfg(feature = "editor")]
use super::super::{
    FoldRange,
    decorations::{TextDecoration, TextDecorationCollection, normalize},
};
use super::{
    element::{EditorScrollbarSnapshot, RIGHT_MARGIN, TextElement},
    mask_pattern::{MaskPattern, normalize_number_input},
    mode::InputMode,
    number_input,
    number_input::{NumberStep, StepAction},
};

/// 回车动作，带修饰键信息。
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = input, no_json)]
pub struct Enter {
    /// 是否为辅助回车确认。
    pub secondary: bool,
    /// 回车时是否按住 Shift。
    pub shift: bool,
}

impl Enter {
    /// 判断给定的 `action` 是否为主回车动作（`secondary: false`），
    /// 不关心是否按住 Shift。
    pub fn is_primary(action: &dyn Action) -> bool {
        action.partial_eq(&Enter {
            secondary: false,
            shift: false,
        }) || action.partial_eq(&Enter {
            secondary: false,
            shift: true,
        })
    }
}

actions!(
    input,
    [
        Backspace,
        Delete,
        DeleteToBeginningOfLine,
        DeleteToEndOfLine,
        DeleteToPreviousWordStart,
        DeleteToNextWordEnd,
        Indent,
        Outdent,
        IndentInline,
        OutdentInline,
        MoveUp,
        MoveDown,
        MoveLeft,
        MoveRight,
        MoveHome,
        MoveEnd,
        MovePageUp,
        MovePageDown,
        SelectAll,
        SelectToStartOfLine,
        SelectToEndOfLine,
        SelectToStart,
        SelectToEnd,
        SelectToPreviousWordStart,
        SelectToNextWordEnd,
        ShowCharacterPalette,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        MoveToStartOfLine,
        MoveToEndOfLine,
        MoveToStart,
        MoveToEnd,
        MoveToPreviousWord,
        MoveToNextWord,
        Escape,
        // 编辑器动作（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        CopyLine,
        #[cfg(feature = "editor")]
        DeleteLine,
        #[cfg(feature = "editor")]
        MoveLineUp,
        #[cfg(feature = "editor")]
        MoveLineDown,
        #[cfg(feature = "editor")]
        ToggleLineComment,
        #[cfg(feature = "editor")]
        JoinLines,
        #[cfg(feature = "editor")]
        AddCursorAbove,
        #[cfg(feature = "editor")]
        AddCursorBelow,
    ]
);

/// 输入事件。
#[derive(Clone)]
pub enum InputEvent {
    /// 文本内容发生变化。
    Change,
    /// 按下回车键。
    PressEnter {
        /// 是否按住 secondary 修饰键（如 Alt）。
        secondary: bool,
        /// 是否按住 Shift 键。
        shift: bool,
    },
    /// 获得焦点。
    Focus,
    /// 失去焦点。
    Blur,
}

pub(crate) const CONTEXT: &str = "Input";

/// 初始化输入组件，注册全局按键绑定。
pub(crate) fn init(cx: &mut App) {
    let mut bindings = vec![
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("shift-backspace", Backspace, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        KeyBinding::new("shift-delete", Delete, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-backspace", DeleteToBeginningOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-delete", DeleteToEndOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-delete", DeleteToNextWordEnd, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-delete", DeleteToNextWordEnd, Some(CONTEXT)),
        KeyBinding::new(
            "enter",
            Enter {
                secondary: false,
                shift: false,
            },
            Some(CONTEXT),
        ),
        KeyBinding::new(
            "shift-enter",
            Enter {
                secondary: false,
                shift: true,
            },
            Some(CONTEXT),
        ),
        KeyBinding::new(
            "secondary-enter",
            Enter {
                secondary: true,
                shift: false,
            },
            Some(CONTEXT),
        ),
        KeyBinding::new("escape", Escape, Some(CONTEXT)),
        KeyBinding::new("up", MoveUp, Some(CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(CONTEXT)),
        KeyBinding::new("pageup", MovePageUp, Some(CONTEXT)),
        KeyBinding::new("pagedown", MovePageDown, Some(CONTEXT)),
        KeyBinding::new("tab", IndentInline, Some(CONTEXT)),
        KeyBinding::new("shift-tab", OutdentInline, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-]", Indent, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-]", Indent, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-[", Outdent, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-[", Outdent, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        KeyBinding::new("shift-up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("shift-down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("home", MoveHome, Some(CONTEXT)),
        KeyBinding::new("end", MoveEnd, Some(CONTEXT)),
        KeyBinding::new("shift-home", SelectToStartOfLine, Some(CONTEXT)),
        KeyBinding::new("shift-end", SelectToEndOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-shift-a", SelectToStartOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-shift-e", SelectToEndOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("shift-cmd-left", SelectToStartOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("shift-cmd-right", SelectToEndOfLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-shift-left", SelectToPreviousWordStart, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-left", SelectToPreviousWordStart, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-shift-right", SelectToNextWordEnd, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-right", SelectToNextWordEnd, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-a", MoveHome, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-left", MoveHome, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-e", MoveEnd, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-right", MoveEnd, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-up", MoveToStart, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-down", MoveToEnd, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-left", MoveToPreviousWord, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-right", MoveToNextWord, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-left", MoveToPreviousWord, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-right", MoveToNextWord, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-up", SelectToStart, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-down", SelectToEnd, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-y", Redo, Some(CONTEXT)),
    ];
    // 编辑器动作键位（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    bindings.extend([
        KeyBinding::new("shift-alt-down", CopyLine, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-k", DeleteLine, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-k", DeleteLine, Some(CONTEXT)),
        KeyBinding::new("alt-up", MoveLineUp, Some(CONTEXT)),
        KeyBinding::new("alt-down", MoveLineDown, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-/", ToggleLineComment, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-/", ToggleLineComment, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-j", JoinLines, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-j", JoinLines, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-alt-up", AddCursorAbove, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-alt-up", AddCursorAbove, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-alt-down", AddCursorBelow, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-alt-down", AddCursorBelow, Some(CONTEXT)),
    ]);
    cx.bind_keys(bindings);

    number_input::init(cx);
}

/// InputState 用于保存 [`super::Input`] 的编辑状态。
pub struct InputState {
    pub(super) focus_handle: FocusHandle,
    pub(crate) mode: InputMode,
    /// 共享文本核（文本/选区/历史/装饰/IME 标记，P1 抽核）。
    pub(crate) core: TextCore,
    pub(crate) display_map: DisplayMap,
    pub(super) blink_cursor: Entity<BlinkCursor>,
    pub(super) loading: bool,
    /// 用于记录拖拽移动时保持的单词选择范围。
    pub(crate) selected_word_range: Option<Selection>,
    pub(crate) last_layout: Option<LastLayout>,
    pub(super) last_cursor: Option<usize>,
    /// 输入容器边界。
    pub(crate) input_bounds: Bounds<Pixels>,
    /// 文本边界。
    pub(super) last_bounds: Option<Bounds<Pixels>>,
    pub(super) last_selected_range: Option<Selection>,
    pub(super) selecting: bool,
    pub(super) size: ElementSize,
    pub(crate) disabled: bool,
    pub(crate) masked: bool,
    /// 掩码状态是否被显式设置（通过 [`Self::masked`] 或 [`Self::set_masked`]），
    /// 用于让 [`Input`](super::Input) 在密码内容类型下仅在用户未显式选择时
    /// 才应用默认掩码，避免覆盖切换按钮的选择。
    pub(super) masked_set: bool,
    pub(super) clean_on_escape: bool,
    pub(super) submit_on_enter: bool,
    pub(super) soft_wrap: bool,
    pub(super) wrapping_indent: WrappingIndent,
    /// 参见 [`Self::scroll_beyond_last_line`]。
    pub(super) scroll_beyond_last_line: Option<usize>,
    /// 参见 [`Self::cursor_surrounding_lines`]。
    pub(super) cursor_surrounding_lines: Option<usize>,
    pub(super) show_whitespaces: bool,
    /// 该标记告知渲染器更倾向于当前可视行的末尾。
    pub(crate) cursor_line_end_affinity: bool,
    pub(super) pattern: Option<Regex>,
    pub(super) validate: Option<Box<dyn Fn(&str, &mut Context<Self>) -> bool + 'static>>,
    /// [`super::NumberInput`] 的步进策略。参见 [`Self::step`] 和 [`Self::step_by`]。
    pub(super) number_step: Option<NumberStep>,
    /// [`super::NumberInput`] 的最小值。参见 [`Self::min`]。
    pub(super) number_min: Option<f64>,
    /// [`super::NumberInput`] 的最大值。参见 [`Self::max`]。
    pub(super) number_max: Option<f64>,
    pub(crate) scroll_handle: ScrollHandle,
    /// 待下次布局时应用的滚动偏移。
    pub(crate) deferred_scroll_offset: Option<Point<Pixels>>,
    /// 可滚动内容的大小。
    pub(crate) scroll_size: crate::Size<Pixels>,
    pub(super) editor_scrollbar_paddings: Cell<Edges<Pixels>>,
    pub(super) editor_scrollbar_snapshot: Cell<Option<EditorScrollbarSnapshot>>,
    pub(super) text_align: TextAlign,
    /// 语法高亮器（`set_highlighter` 设置；`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(super) highlighter: Option<Box<dyn Highlighter>>,
    /// 高亮装饰集合（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    highlight_collection: Option<TextDecorationCollection>,

    /// 用于格式化输入文本的掩码模式。
    pub(crate) mask_pattern: MaskPattern,
    /// `mask_pattern` 是否被显式设置（通过 [`Self::mask_pattern`] 或
    /// [`Self::set_mask_pattern`]），以让 [`super::NumberInput`] 仅在用户未显式
    /// 选择时应用其默认掩码。
    pub(super) mask_pattern_set: bool,
    pub(super) placeholder: SharedString,
    /// 行注释符（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) line_comment_prefix: SharedString,

    /// 标记文本是否有待处理的更新。
    ///
    /// 若为 true，将在渲染前重新准备文本。
    _pending_update: bool,
    /// 标记是否应发送 InputEvents。
    pub(super) emit_events: bool,

    /// 用于记住光标所在水平列（x 坐标），以在上下移动时保持列位置。
    ///
    /// 第一个元素是 x 坐标（Pixels），优先使用。
    /// 第二个元素是列（usize），回退使用。
    pub(crate) preferred_column: Option<(Pixels, usize)>,
    _subscriptions: Vec<Subscription>,

    pub(super) auto_scroll: AutoScroll,
    /// 右键菜单总开关（默认启用，见 `input_ui/context_menu.rs`）。
    pub(crate) context_menu_enabled: bool,
    /// 默认菜单后的追加项（`Input::context_menu_extra` 或 state 层设置写入）。
    pub(crate) context_menu_extra: Option<InputContextMenuBuilder>,
    /// 完全接管菜单（`Input::context_menu_override` 或 state 层设置写入）。
    pub(crate) context_menu_override: Option<InputContextMenuBuilder>,
    /// 只读模式：保持正常样式，允许移动/选择/复制，拦截一切用户编辑。
    pub(crate) read_only: bool,
    /// 括号自动闭合：`None` 为自动（多行开、单行关），`Some` 显式覆盖
    ///（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) auto_close_pairs: Option<bool>,
    /// 括号匹配高亮开关（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) bracket_match_enabled: bool,
    /// 括号匹配高亮的装饰集合（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) bracket_match_collection: Option<TextDecorationCollection>,
    /// 括号彩虹开关（`editor` feature 门控，默认关，见 O5）。
    #[cfg(feature = "editor")]
    pub(crate) bracket_rainbow_enabled: bool,
    /// 括号彩虹的装饰集合（`editor` feature 门控；后于匹配集合创建，画在上层）。
    #[cfg(feature = "editor")]
    pub(crate) bracket_rainbow_collection: Option<TextDecorationCollection>,
    /// 当前行高亮开关（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) current_line_highlight: bool,
    /// 当前行高亮的装饰集合（`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) current_line_collection: Option<TextDecorationCollection>,
    /// 标尺列（字符数；空即关，`editor` feature 门控，见 O5）。
    #[cfg(feature = "editor")]
    pub(crate) rulers: Vec<usize>,
    /// 标尺颜色（`None` 跟主题边框色；`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) ruler_color: Option<Hsla>,
    /// 行内提示总开关（`editor` feature 门控，默认关，见 `editor/inlay_hints.rs`）。
    #[cfg(feature = "editor")]
    pub(crate) inlay_hints_enabled: bool,
    /// 已存行内提示（渲染源；`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) inlay_hints: Vec<super::super::editor::inlay_hints::InlayHint>,
    /// gutter 列总开关（默认关；`editor` feature 门控，见 `editor/gutter.rs`）。
    #[cfg(feature = "editor")]
    pub(crate) gutter_column_enabled: bool,
    /// gutter provider 表（具名 + 独立开关；`editor` feature 门控）。
    #[cfg(feature = "editor")]
    pub(crate) gutter_markers: Vec<super::super::editor::gutter::GutterMarkerEntry>,
    /// 主光标之外的额外光标（多光标编辑，见 `input_ui/multicursor.rs`；
    /// 字段常驻（读点太多），行为由 `editor` feature 门控）。
    pub(crate) extra_selections: Vec<Selection>,
    /// Vim 运行时状态（模式/前缀/锚点/寄存器；`editor` feature 门控，见 O3）。
    #[cfg(feature = "editor")]
    pub(crate) vim: super::super::editor::vim::VimState,
}

impl EventEmitter<InputEvent> for InputState {}

impl InputState {
    /// 以默认 [`InputMode::SingleLine`] 模式创建输入状态。
    ///
    /// 参见 [`Self::multi_line`]、[`Self::auto_grow`] 设置其他模式。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);
        let blink_cursor = cx.new(|_| BlinkCursor::new());

        let _subscriptions = vec![
            // 观察闪烁光标，以便在其变化时重绘视图。
            cx.observe(&blink_cursor, |_, _, cx| cx.notify()),
            // 窗口激活时闪烁光标，非激活时暂停。
            cx.observe_window_activation(window, |input, window, cx| {
                if window.is_window_active() {
                    let focus_handle = input.focus_handle.clone();
                    if focus_handle.is_focused(window) {
                        input.blink_cursor.update(cx, |blink_cursor, cx| {
                            blink_cursor.start(cx);
                        });
                    }
                }
            }),
            cx.on_focus(&focus_handle, window, Self::on_focus),
            cx.on_blur(&focus_handle, window, Self::on_blur),
        ];

        let text_style = window.text_style();

        Self {
            focus_handle: focus_handle.clone(),
            core: TextCore::new(),
            display_map: DisplayMap::new(
                text_style.font(),
                text_style.font_size.to_pixels(window.rem_size()),
                None,
            ),
            blink_cursor,
            selected_word_range: None,
            input_bounds: Bounds::default(),
            selecting: false,
            disabled: false,
            masked: false,
            masked_set: false,
            clean_on_escape: false,
            submit_on_enter: false,
            soft_wrap: true,
            wrapping_indent: WrappingIndent::default(),
            scroll_beyond_last_line: None,
            cursor_surrounding_lines: None,
            show_whitespaces: false,
            loading: false,
            pattern: None,
            validate: None,
            number_step: Some(NumberStep::Fixed(1.)),
            number_min: None,
            number_max: None,
            mode: InputMode::default(),
            last_layout: None,
            last_bounds: None,
            last_selected_range: None,
            last_cursor: None,
            scroll_handle: ScrollHandle::new(),
            scroll_size: crate::size(px(0.), px(0.)),
            editor_scrollbar_paddings: Cell::new(Edges {
                top: px(0.),
                right: px(0.),
                bottom: px(0.),
                left: px(0.),
            }),
            editor_scrollbar_snapshot: Cell::new(None),
            deferred_scroll_offset: None,
            preferred_column: None,
            placeholder: SharedString::default(),
            #[cfg(feature = "editor")]
            line_comment_prefix: "//".into(),
            mask_pattern: MaskPattern::default(),
            mask_pattern_set: false,
            text_align: TextAlign::Left,
            #[cfg(feature = "editor")]
            highlighter: None,
            #[cfg(feature = "editor")]
            highlight_collection: None,
            emit_events: true,
            size: ElementSize::default(),
            _subscriptions,
            _pending_update: false,
            cursor_line_end_affinity: false,
            auto_scroll: AutoScroll::default(),
            context_menu_enabled: true,
            context_menu_extra: None,
            context_menu_override: None,
            read_only: false,
            #[cfg(feature = "editor")]
            auto_close_pairs: None,
            #[cfg(feature = "editor")]
            bracket_match_enabled: true,
            #[cfg(feature = "editor")]
            bracket_match_collection: None,
            #[cfg(feature = "editor")]
            bracket_rainbow_enabled: false,
            #[cfg(feature = "editor")]
            bracket_rainbow_collection: None,
            #[cfg(feature = "editor")]
            current_line_highlight: true,
            #[cfg(feature = "editor")]
            current_line_collection: None,
            #[cfg(feature = "editor")]
            rulers: Vec::new(),
            #[cfg(feature = "editor")]
            ruler_color: None,
            #[cfg(feature = "editor")]
            inlay_hints_enabled: false,
            #[cfg(feature = "editor")]
            inlay_hints: Vec::new(),
            #[cfg(feature = "editor")]
            gutter_column_enabled: false,
            #[cfg(feature = "editor")]
            gutter_markers: Vec::new(),
            extra_selections: Vec::new(),
            #[cfg(feature = "editor")]
            vim: super::super::editor::vim::VimState::default(),
        }
    }

    /// 设置输入框为多行模式。
    ///
    /// 默认行数为 2。
    pub fn multi_line(mut self, multi_line: bool) -> Self {
        self.mode = self.mode.multi_line(multi_line);
        self
    }

    /// 设置输入框为 [`InputMode::AutoGrow`] 模式，并限制最小、最大行数。
    pub fn auto_grow(mut self, min_rows: usize, max_rows: usize) -> Self {
        self.mode = InputMode::auto_grow(min_rows, max_rows);
        self
    }

    /// 设置占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置是否启用代码折叠，仅多行生效（单行忽略）。
    ///
    /// 默认：false（编辑器经 `EditorState` 显式打开）。
    pub fn folding(mut self, folding: bool) -> Self {
        debug_assert!(self.mode.is_multi_line());
        if let InputMode::PlainText { folding: f, .. } = &mut self.mode {
            *f = folding;
        }
        self
    }

    /// 运行时设置代码折叠，仅多行生效（单行忽略）。
    ///
    /// 禁用时会清除所有已存在的折叠。
    pub fn set_folding(&mut self, folding: bool, _: &mut Window, cx: &mut Context<Self>) {
        debug_assert!(self.mode.is_multi_line());
        if let InputMode::PlainText { folding: f, .. } = &mut self.mode {
            *f = folding;
        }
        if !folding {
            self.display_map.clear_folds();
        }
        cx.notify();
    }

    /// 设置是否显示行号，仅多行生效（单行忽略）。
    pub fn line_number(mut self, line_number: bool) -> Self {
        debug_assert!(self.mode.is_multi_line());
        if let InputMode::PlainText { line_number: l, .. } = &mut self.mode {
            *l = line_number;
        }
        self
    }

    /// 运行时设置行号，仅多行生效（单行忽略）。
    pub fn set_line_number(&mut self, line_number: bool, _: &mut Window, cx: &mut Context<Self>) {
        debug_assert!(self.mode.is_multi_line());
        if let InputMode::PlainText { line_number: l, .. } = &mut self.mode {
            *l = line_number;
        }
        cx.notify();
    }

    /// 设置多行文本框的行数。
    ///
    /// 仅当 `multi_line` 为 true 时生效。
    ///
    /// 默认：2
    pub fn rows(mut self, rows: usize) -> Self {
        match &mut self.mode {
            InputMode::PlainText { rows: r, .. } => *r = rows,
            InputMode::AutoGrow {
                max_rows: max_r,
                rows: r,
                ..
            } => {
                *r = rows;
                *max_r = rows;
            }
        }
        self
    }

    /// 设置占位文本。
    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    #[cfg(feature = "editor")]
    /// 设置行注释符（`ToggleLineComment` 用，默认 `//`）。
    ///
    /// 按语言设置，如 Python 传 `"#"`、Lua 传 `"--"`、Rust 传 `"//"`。
    pub fn line_comment_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.line_comment_prefix = prefix.into();
        self
    }

    #[cfg(feature = "editor")]
    /// 设置行注释符（创建后修改）。
    pub fn set_line_comment_prefix(
        &mut self,
        prefix: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.line_comment_prefix = prefix.into();
        cx.notify();
    }

    /// 设置只读模式（builder 版，创建时链式调用）。
    ///
    /// 只读保持正常样式（不像禁用那样变灰），允许移动光标/选择/复制，
    /// 但拦截一切用户编辑（键入/粘贴/剪切/撤销/行操作/回车换行）。
    /// 程序化写入（`insert`/`replace`/`set_value`）不受影响。
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// 设置只读模式（创建后修改）。
    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    /// 是否只读。
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    #[cfg(feature = "editor")]
    /// 设置括号自动闭合（builder 版，创建时链式调用）。
    ///
    /// 默认自动：多行开启、单行关闭；显式传值覆盖自动规则。
    pub fn auto_close_pairs(mut self, enabled: bool) -> Self {
        self.auto_close_pairs = Some(enabled);
        self
    }

    #[cfg(feature = "editor")]
    /// 设置括号自动闭合（创建后修改，传 `None` 恢复自动规则）。
    pub fn set_auto_close_pairs(&mut self, enabled: Option<bool>, cx: &mut Context<Self>) {
        self.auto_close_pairs = enabled;
        cx.notify();
    }

    #[cfg(feature = "editor")]
    /// 设置括号匹配高亮开关（builder 版，创建时链式调用，默认开）。
    pub fn bracket_match(mut self, enabled: bool) -> Self {
        self.bracket_match_enabled = enabled;
        self
    }

    #[cfg(feature = "editor")]
    /// 设置括号匹配高亮开关（创建后修改，关闭时立即清除高亮）。
    pub fn set_bracket_match_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.bracket_match_enabled = enabled;
        if !enabled {
            self.clear_bracket_match(cx);
        }
        cx.notify();
    }

    #[cfg(feature = "editor")]
    /// 设置括号彩虹开关（builder 版，创建时链式调用，默认关；O5）。
    ///
    /// 括号字色按嵌套深度轮转（主题调色轮）；与匹配高亮共存时匹配 accent 盖顶。
    pub fn bracket_rainbow(mut self, enabled: bool) -> Self {
        self.bracket_rainbow_enabled = enabled;
        self
    }

    #[cfg(feature = "editor")]
    /// 设置括号彩虹开关（创建后修改，关闭时立即清除；打开后按当前文本刷新）。
    pub fn set_bracket_rainbow_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.bracket_rainbow_enabled = enabled;
        if enabled {
            self.refresh_bracket_rainbow(cx);
        } else {
            self.clear_bracket_rainbow(cx);
        }
        cx.notify();
    }

    #[cfg(feature = "editor")]
    /// 设置当前行高亮开关（builder 版，创建时链式调用，默认开）。
    pub fn current_line_highlight(mut self, enabled: bool) -> Self {
        self.current_line_highlight = enabled;
        self
    }

    #[cfg(feature = "editor")]
    /// 设置当前行高亮开关（创建后修改，关闭时立即清除高亮）。
    pub fn set_current_line_highlight(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.current_line_highlight = enabled;
        if !enabled {
            self.clear_current_line(cx);
        }
        cx.notify();
    }

    /// 查找给定偏移量所在的行与子行，以及该偏移量在子行内的位置。
    ///
    /// 返回：
    ///
    /// - 包含该偏移量的行索引（0 基）。
    /// - 包含该偏移量的子行索引（0 基）。
    /// - 该偏移量的位置。
    pub(super) fn line_and_position_for_offset(
        &self,
        offset: usize,
    ) -> (usize, usize, Option<Point<Pixels>>) {
        let Some(last_layout) = &self.last_layout else {
            return (0, 0, None);
        };
        let line_height = last_layout.line_height;

        let mut y_offset = last_layout.visible_top;
        for (vi, line) in last_layout.lines.iter().enumerate() {
            let prev_lines_offset = last_layout.visible_line_byte_offsets[vi];
            let local_offset = offset.saturating_sub(prev_lines_offset);
            if let Some(pos) = line.position_for_index(local_offset, last_layout, false) {
                let sub_line_index = (pos.y / line_height) as usize;
                let adjusted_pos = point(pos.x + last_layout.line_number_width, pos.y + y_offset);
                return (vi, sub_line_index, Some(adjusted_pos));
            }

            y_offset += line.size(line_height).height;
        }
        (0, 0, None)
    }

    /// 设置输入框的文本。
    ///
    /// 单行输入时光标置于文本末尾，同时视图滚动回开头，使长值显示其开头
    /// 而非结尾（与 HTML `<input>` 一致）。多行输入将选择重置为 `0..0`。
    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.core.history.ignore = true;
        self.emit_events = false;
        self.replace_text(value, window, cx);
        self.core.history.ignore = false;
        self.emit_events = true;

        self.reset_selection();
        self.request_text_prepare();
        self.reset_scroll_to_start();

        self.core.history.clear();
        cx.notify();
    }

    /// 替换整个文本内容并保留撤销历史。
    ///
    /// 与 [`set_value`](Self::set_value) 不同，该方法会在撤销栈中记录替换，
    /// 允许用户撤销/重做该更改。单行输入时选择置于新文本末尾，多行输入时
    /// 清除选择（0..0）。
    ///
    /// 当以编程方式替换全文但仍希望用户可撤销时使用——例如格式化。
    pub fn replace_all(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text(text, window, cx);
        self.reset_selection();
        self.request_text_prepare();
        self.reset_scroll_to_start();

        cx.notify();
    }

    /// 在当前光标位置插入文本。
    ///
    /// 光标将移动到插入文本的末尾。
    pub fn insert(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_disabled = self.disabled;
        self.disabled = false;
        let text: SharedString = text.into();
        let range_utf16 = self.range_to_utf16(&(self.cursor()..self.cursor()));
        self.replace_text_in_range_silent(Some(range_utf16), &text, window, cx);
        self.core.selected_range =
            (self.core.selected_range.end..self.core.selected_range.end).into();
        self.disabled = was_disabled;
    }

    /// 在当前光标位置替换文本。
    ///
    /// 光标将移动到替换文本的末尾。
    pub fn replace(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_disabled = self.disabled;
        self.disabled = false;
        let text: SharedString = text.into();
        self.replace_text_in_range_silent(None, &text, window, cx);
        self.core.selected_range =
            (self.core.selected_range.end..self.core.selected_range.end).into();
        self.disabled = was_disabled;
    }

    fn replace_text(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_disabled = self.disabled;
        self.disabled = false;
        let text: SharedString = text.into();
        let range = 0..self.core.text.chars().map(|c| c.len_utf16()).sum();
        self.replace_text_in_range_silent(Some(range), &text, window, cx);
        self.disabled = was_disabled;
    }

    fn reset_selection(&mut self) {
        // 单行输入时光标置于文本末尾（与 HTML `<input>` 一致）；
        // 多行输入将选择重置为 `0..0`。
        if self.mode.is_single_line() {
            let end = self.core.text.len();
            self.core.selected_range = (end..end).into();
        } else {
            self.core.selected_range.clear();
        }
    }

    /// 全量替换后下次 render 前重备 display_map 文本。
    fn request_text_prepare(&mut self) {
        self._pending_update = true;
    }

    fn reset_scroll_to_start(&mut self) {
        // 将滚动移到开头。单行时光标在末尾，因此设置延迟偏移以在下次绘制
        // 时覆盖光标跟随滚动，保持开头可见。
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
        if self.mode.is_single_line() {
            self.deferred_scroll_offset = Some(point(px(0.), px(0.)));
        }
    }

    /// 设置密码掩码状态。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式。
    pub fn masked(mut self, masked: bool) -> Self {
        debug_assert!(self.mode.is_single_line());
        self.masked = masked;
        self.masked_set = true;
        self
    }

    /// 设置输入框的密码掩码状态。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式。
    pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
        debug_assert!(self.mode.is_single_line());
        self.masked = masked;
        self.masked_set = true;
        cx.notify();
    }

    /// 设为 true 时按 Escape 键清空输入。
    pub fn clean_on_escape(mut self) -> Self {
        self.clean_on_escape = true;
        self
    }

    /// 设为 true 时在多行模式下将 `Enter` 视为提交动作，而 `Shift+Enter` 插入换行。
    ///
    /// 默认是 `false`（`Enter` 和 `Shift+Enter` 都插入换行）。
    pub fn submit_on_enter(mut self, submit: bool) -> Self {
        self.submit_on_enter = submit;
        self
    }

    /// 设置多行输入的软换行模式，默认 true。
    pub fn soft_wrap(mut self, wrap: bool) -> Self {
        debug_assert!(self.mode.is_multi_line());
        self.soft_wrap = wrap;
        self
    }

    /// 设置是否显示空白字符。
    pub fn show_whitespaces(mut self, show: bool) -> Self {
        self.show_whitespaces = show;
        self
    }

    /// 设置软换行连续行的缩进方式，默认 [`WrappingIndent::Same`]。
    pub fn wrapping_indent(mut self, wrapping_indent: WrappingIndent) -> Self {
        debug_assert!(self.mode.is_multi_line());
        self.wrapping_indent = wrapping_indent;
        self
    }

    /// 更新多行输入的软换行模式，默认 true。
    pub fn set_soft_wrap(&mut self, wrap: bool, _: &mut Window, cx: &mut Context<Self>) {
        debug_assert!(self.mode.is_multi_line());
        self.soft_wrap = wrap;
        if wrap {
            let wrap_width = self
                .last_layout
                .as_ref()
                .and_then(|b| b.wrap_width)
                .unwrap_or(self.input_bounds.size.width);

            self.display_map.on_layout_changed(Some(wrap_width), cx);

            // 将滚动重置到最左 0
            let mut offset = self.scroll_handle.offset();
            offset.x = px(0.);
            self.scroll_handle.set_offset(offset);
        } else {
            self.display_map.on_layout_changed(None, cx);
        }
        cx.notify();
    }

    /// 更新是否显示空白字符。
    pub fn set_show_whitespaces(&mut self, show: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.show_whitespaces = show;
        cx.notify();
    }

    /// 更新软换行连续行的缩进方式。
    pub fn set_wrapping_indent(
        &mut self,
        wrapping_indent: WrappingIndent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.wrapping_indent = wrapping_indent;
        self.display_map.set_wrapping_indent(wrapping_indent, cx);
        cx.notify();
    }

    /// 最后一行内容下方保留的空行数（"越过最后一行滚动"），仅代码编辑器模式。
    /// 对应 VSCode 的 `editor.scrollBeyondLastLine` / Zed 的 `scroll_beyond_last_line`。
    ///
    /// - `None`（默认）：半个视口，下限为 [`BOTTOM_MARGIN_ROWS`] 行高。
    /// - `Some(0)`：无尾部空间；光标在滚动到最大时与最后一行平齐。
    /// - `Some(n)`：恰好 `n` 行。
    pub fn scroll_beyond_last_line(mut self, rows: Option<usize>) -> Self {
        self.scroll_beyond_last_line = rows;
        self
    }

    /// 构造后更新 [`Self::scroll_beyond_last_line`]。
    pub fn set_scroll_beyond_last_line(
        &mut self,
        rows: Option<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.scroll_beyond_last_line == rows {
            return;
        }
        self.scroll_beyond_last_line = rows;
        cx.notify();
    }

    /// 光标距视口上下边缘保持的最小行数，之后才触发自动滚动。
    /// 对应 VSCode 的 `editor.cursorSurroundingLines` / Zed 的 `vertical_scroll_margin`。
    /// 与 [`Self::scroll_beyond_last_line`] 正交，后者控制空区域大小。
    ///
    /// - `None`（默认）：[`BOTTOM_MARGIN_ROWS`] 行，小视口回退为一行。
    /// - `Some(n)`：恰好 `n` 行，钳制到半个视口。
    pub fn cursor_surrounding_lines(mut self, lines: Option<usize>) -> Self {
        self.cursor_surrounding_lines = lines;
        self
    }

    /// 构造后更新 [`Self::cursor_surrounding_lines`]。
    pub fn set_cursor_surrounding_lines(
        &mut self,
        lines: Option<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.cursor_surrounding_lines == lines {
            return;
        }
        self.cursor_surrounding_lines = lines;
        cx.notify();
    }

    /// 设置输入框的正则表达式模式。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式。
    pub fn pattern(mut self, pattern: Regex) -> Self {
        debug_assert!(self.mode.is_single_line());
        self.pattern = Some(pattern);
        self
    }

    /// 设置输入框的正则表达式模式（引用方式）。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式。
    pub fn set_pattern(&mut self, pattern: Regex, _window: &mut Window, _cx: &mut Context<Self>) {
        debug_assert!(self.mode.is_single_line());
        self.pattern = Some(pattern);
    }

    /// 设置输入框的验证函数。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式。
    pub fn validate(mut self, f: impl Fn(&str, &mut Context<Self>) -> bool + 'static) -> Self {
        debug_assert!(self.mode.is_single_line());
        self.validate = Some(Box::new(f));
        self
    }

    /// 设置 [`super::NumberInput`] 的步进值。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式搭配 [`super::NumberInput`]。
    ///
    /// 若设置了 `step`、`min`、`max` 中任一，则 [`super::NumberInput`] 将内部更新
    /// 数值（按 `step` 步进，默认 1，并钳制到 `min`/`max` 范围，发送
    /// [`InputEvent::Change`]）而非发送 [`super::NumberInputEvent::Step`]。
    ///
    /// 参见 [`Self::step_by`] 基于当前值计算步进。
    pub fn step(mut self, step: impl Into<NumberStep>) -> Self {
        debug_assert!(self.mode.is_single_line());
        self.number_step = Some(step.into());
        self
    }

    /// 设置根据当前值和方向计算步进的函数，例如按范围变化的步长。
    ///
    /// 当前值是步进前的值；空值或非法值按 0 处理。在范围边界需要按方向区分
    /// 步长时，可利用 [`StepAction`] 判断是递增还是递减。
    ///
    /// 这是 `step(NumberStep::by_value(f))` 的简写。参见 [`Self::step`]。
    ///
    /// 闭包接收 [`Context<Self>`] 以读取或更新其他实体，但不得重入所属的
    /// [`InputState`]（步进期间它被可变借用）。
    pub fn step_by(
        mut self,
        f: impl Fn(f64, StepAction, &mut Context<Self>) -> f64 + 'static,
    ) -> Self {
        debug_assert!(self.mode.is_single_line());
        self.number_step = Some(NumberStep::by_value(f));
        self
    }

    /// 设置 [`super::NumberInput`] 的最小值。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式搭配 [`super::NumberInput`]。
    ///
    /// 数值在步进和失焦时被钳制到最小值（仅当钳制后的值通过
    /// `pattern`/`validate` 检查）。参见 [`Self::step`]。
    pub fn min(mut self, min: f64) -> Self {
        debug_assert!(self.mode.is_single_line());
        self.number_min = Some(min);
        self
    }

    /// 设置 [`super::NumberInput`] 的最大值。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式搭配 [`super::NumberInput`]。
    ///
    /// 数值在步进和失焦时被钳制到最大值（仅当钳制后的值通过
    /// `pattern`/`validate` 检查）。参见 [`Self::step`]。
    pub fn max(mut self, max: f64) -> Self {
        debug_assert!(self.mode.is_single_line());
        self.number_max = Some(max);
        self
    }

    /// 构造后更新步进值，`None` 回退为发送 [`super::NumberInputEvent::Step`]
    /// （若 `min`、`max` 均未设置）。
    ///
    /// 参见 [`Self::step`] 和 [`Self::step_by`]。
    pub fn set_step(
        &mut self,
        step: impl Into<Option<NumberStep>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        debug_assert!(self.mode.is_single_line());
        self.number_step = step.into();
    }

    /// 构造后更新最小值。参见 [`Self::min`]。
    pub fn set_min(&mut self, min: Option<f64>, _: &mut Window, _: &mut Context<Self>) {
        debug_assert!(self.mode.is_single_line());
        self.number_min = min;
    }

    /// 构造后更新最大值。参见 [`Self::max`]。
    pub fn set_max(&mut self, max: Option<f64>, _: &mut Window, _: &mut Context<Self>) {
        debug_assert!(self.mode.is_single_line());
        self.number_max = max;
    }

    /// 设为 true 以在输入框右侧显示加载指示器。
    ///
    /// 仅 [`InputMode::SingleLine`] 模式。
    pub fn set_loading(&mut self, loading: bool, _: &mut Window, cx: &mut Context<Self>) {
        debug_assert!(self.mode.is_single_line());
        self.loading = loading;
        cx.notify();
    }

    /// 设置输入框的默认值。
    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        let text: SharedString = value.into();
        self.core.text = Rope::from(self.normalize_input(&text).as_ref());
        // 注意：这里不能调用 display_map.set_text，因为它需要 cx。
        // 文本将在 element.rs 的 prepare_if_need 阶段设置。
        self._pending_update = true;
        self
    }

    /// 返回输入框的值。
    pub fn value(&self) -> SharedString {
        SharedString::new(self.core.text.to_string())
    }

    /// 返回输入框中被用户选中的那部分值。
    pub fn selected_value(&self) -> SharedString {
        SharedString::new(self.selected_text().to_string())
    }

    /// 返回去除掩码后的值。
    pub fn unmask_value(&self) -> SharedString {
        self.mask_pattern.unmask(&self.core.text.to_string()).into()
    }

    /// 返回输入框的文本 [`Rope`]。
    pub fn text(&self) -> &Rope {
        &self.core.text
    }

    /// 返回光标的（0 基）[`Position`]。
    pub fn cursor_position(&self) -> Position {
        let offset = self.cursor();
        self.core.text.offset_to_position(offset)
    }

    /// 设置光标的（0 基）[`Position`]。
    ///
    /// 将光标移动到指定行列，并更新选择范围。
    pub fn set_cursor_position(
        &mut self,
        position: impl Into<Position>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position: Position = position.into();
        let offset = self.core.text.position_to_offset(&position);

        self.move_to(offset, None, cx);
        self.update_preferred_column();
        self.focus(window, cx);
    }

    /// 聚焦输入框。
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        self.blink_cursor.update(cx, |cursor, cx| {
            cursor.start(cx);
        });
    }

    /// 刷新输入，使下次渲染重新准备文本，而不仅是重绘。
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self._pending_update = true;
        cx.notify();
    }

    pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor()), cx);
    }

    pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor()), cx);
    }

    pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_single_line() {
            return;
        }
        let offset = self.start_of_line().saturating_sub(1);
        self.select_to(self.previous_boundary(offset), cx);
    }

    pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_single_line() {
            return;
        }
        let offset = (self.end_of_line() + 1).min(self.core.text.len());
        self.select_to(self.next_boundary(offset), cx);
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        // 全选即单选区：先清额外光标。
        let len = self.core.text.len();
        self.set_selected_range(0..len, cx);
    }

    pub(super) fn select_to_start(
        &mut self,
        _: &SelectToStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, cx);
    }

    pub(super) fn select_to_end(
        &mut self,
        _: &SelectToEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let end = self.core.text.len();
        self.select_to(end, cx);
    }

    pub(super) fn select_to_start_of_line(
        &mut self,
        _: &SelectToStartOfLine,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.start_of_line();
        self.select_to(offset, cx);
    }

    pub(super) fn select_to_end_of_line(
        &mut self,
        _: &SelectToEndOfLine,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.end_of_line();
        self.select_to(offset, cx);
    }

    pub(super) fn select_to_previous_word(
        &mut self,
        _: &SelectToPreviousWordStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.previous_start_of_word();
        self.select_to(offset, cx);
    }

    pub(super) fn select_to_next_word(
        &mut self,
        _: &SelectToNextWordEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.next_end_of_word();
        self.select_to(offset, cx);
    }

    /// 返回前一个单词的起始偏移量。
    pub(crate) fn previous_start_of_word(&mut self) -> usize {
        let offset = self.core.selected_range.start;
        let offset = self.offset_from_utf16(self.offset_to_utf16(offset));
        // FIXME: 避免 to_string
        let left_part = self.core.text.slice(0..offset).to_string();

        UnicodeSegmentation::split_word_bound_indices(left_part.as_str())
            .rfind(|(_, s)| !s.trim_start().is_empty())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// 返回下一个单词的结束偏移量。
    pub(crate) fn next_end_of_word(&mut self) -> usize {
        let offset = self.cursor();
        let offset = self.offset_from_utf16(self.offset_to_utf16(offset));
        let right_part = self
            .core
            .text
            .slice(offset..self.core.text.len())
            .to_string();

        UnicodeSegmentation::split_word_bound_indices(right_part.as_str())
            .find(|(_, s)| !s.trim_start().is_empty())
            .map(|(i, s)| offset + i + s.len())
            .unwrap_or(self.core.text.len())
    }

    /// 获取光标所在行的起始字节偏移。
    ///
    /// 软换行激活时，第一次按键到达可视行起点，第二次（已在可视行起点）
    /// 到达逻辑行起点。
    pub(crate) fn start_of_line(&self) -> usize {
        if self.mode.is_single_line() {
            return 0;
        }

        let row = self.core.text.offset_to_point(self.cursor()).row;
        let logical_start = self.core.text.line_start_offset(row);

        if self.soft_wrap && self.mode.is_multi_line() {
            let wrap_point = self.display_map.offset_to_wrap_display_point(self.cursor());
            if let Some(line) = self.display_map.line(row)
                && let Some(range) = line.wrapped_lines.get(wrap_point.local_row)
            {
                let visual_start = logical_start + range.start;
                if self.cursor() != visual_start {
                    return visual_start;
                }
            }
        }

        logical_start
    }

    /// 获取光标所在行的结束字节偏移。
    ///
    /// 软换行激活时，第一次按键到达可视行末尾，第二次（已在可视行末尾）
    /// 到达逻辑行末尾。
    pub(crate) fn end_of_line(&self) -> usize {
        if self.mode.is_single_line() {
            return self.core.text.len();
        }

        let row = self.core.text.offset_to_point(self.cursor()).row;
        let logical_start = self.core.text.line_start_offset(row);
        let logical_end = self.core.text.line_end_offset(row);

        if self.soft_wrap && self.mode.is_multi_line() {
            let wrap_point = self.display_map.offset_to_wrap_display_point(self.cursor());
            if let Some(line) = self.display_map.line(row)
                && let Some(range) = line.wrapped_lines.get(wrap_point.local_row)
            {
                let visual_end = logical_start + range.end;
                if self.cursor() != visual_end {
                    return visual_end;
                }
            }
        }

        logical_end
    }

    /// 获取选择起点或终点所在行的起点（取最小值）。
    ///
    /// 即始终获取选择的第一行。
    pub(crate) fn start_of_line_of_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        if self.mode.is_single_line() {
            return 0;
        }

        let mut offset = self.previous_boundary(
            self.core
                .selected_range
                .start
                .min(self.core.selected_range.end),
        );
        if self.core.text.char_at(offset) == Some('\r') {
            offset += 1;
        }

        let line = self
            .text_for_range(self.range_to_utf16(&(0..offset + 1)), &mut None, window, cx)
            .unwrap_or_default()
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        line
    }

    /// 获取下一行的缩进字符串。
    ///
    /// 计算当前行与下一行的缩进，返回缩进更深的那一个。
    pub(super) fn indent_of_next_line(&mut self) -> String {
        if self.mode.is_single_line() {
            return "".into();
        }

        let mut current_indent = String::new();
        let mut next_indent = String::new();
        let current_line_start_pos = self.start_of_line();
        let next_line_start_pos = self.end_of_line();
        for c in self.core.text.slice(current_line_start_pos..).chars() {
            if !c.is_whitespace() {
                break;
            }
            if c == '\n' || c == '\r' {
                break;
            }
            current_indent.push(c);
        }

        for c in self.core.text.slice(next_line_start_pos..).chars() {
            if !c.is_whitespace() {
                break;
            }
            if c == '\n' || c == '\r' {
                break;
            }
            next_indent.push(c);
        }

        if next_indent.len() > current_indent.len() {
            return next_indent;
        } else {
            return current_indent;
        }
    }

    /// 计算回车插入文本与中间行光标（电缩进）。
    ///
    /// - 光标前（去尾空白后）是 `{`/`[`/`(` 之一：新行多缩一级；
    /// - 且光标后（去首空白后）是对应 `}`/`]`/`)` 之一：拆成三行，
    ///   光标落中间行末（返回 `Some` 偏移），行尾原有文本保持不动；
    /// - 否则普通换行（返回 `None`，光标自然落插入末尾）。
    fn electric_newline(&self, base_indent: &str, cursor: usize) -> (String, Option<usize>) {
        let line_start = self.start_of_line();
        let line_end = self.end_of_line();
        let before = self.core.text.slice(line_start..cursor).to_string();
        let after = self.core.text.slice(cursor..line_end).to_string();
        let opens = before
            .trim_end()
            .chars()
            .next_back()
            .is_some_and(|c| matches!(c, '{' | '[' | '('));
        if !opens {
            return (format!("\n{base_indent}"), None);
        }
        let step = self.mode.tab_size().to_string();
        if after.trim_start().starts_with(['}', ']', ')']) {
            let middle = cursor + 1 + base_indent.len() + step.len();
            (
                format!("\n{base_indent}{step}\n{base_indent}"),
                Some(middle),
            )
        } else {
            (format!("\n{base_indent}{step}"), None)
        }
    }

    pub(crate) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        // 多光标：每处删选区或一个边界单位（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if !self.extra_selections.is_empty() {
                self.multi_delete(true, window, cx);
                self.pause_blink_cursor(cx);
                return;
            }
        }
        // 光标夹在空括号对中间时成对删除（如 `(|)` 一次删 `()`，`editor` 门控）。
        #[cfg(feature = "editor")]
        {
            if self.core.selected_range.is_empty()
                && let Some(pair) = super::super::editor::auto_close::smart_backspace_range(self)
            {
                self.replace_text_in_range_silent(Some(self.range_to_utf16(&pair)), "", window, cx);
                self.pause_blink_cursor(cx);
                return;
            }
        }
        if self.core.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor()), cx)
        }
        self.replace_text_in_range(None, "", window, cx);
        self.pause_blink_cursor(cx);
    }

    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        // 多光标：每处删选区或一个边界单位（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if !self.extra_selections.is_empty() {
                self.multi_delete(false, window, cx);
                self.pause_blink_cursor(cx);
                return;
            }
        }
        if self.core.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor()), cx)
        }
        self.replace_text_in_range(None, "", window, cx);
        self.pause_blink_cursor(cx);
    }

    pub(super) fn delete_to_beginning_of_line(
        &mut self,
        _: &DeleteToBeginningOfLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.core.selected_range.is_empty() {
            self.replace_text_in_range(None, "", window, cx);
            self.pause_blink_cursor(cx);
            return;
        }

        let mut offset = self.start_of_line();
        if offset == self.cursor() {
            offset = offset.saturating_sub(1);
        }
        self.replace_text_in_range_silent(
            Some(self.range_to_utf16(&(offset..self.cursor()))),
            "",
            window,
            cx,
        );
        self.pause_blink_cursor(cx);
    }

    pub(super) fn delete_to_end_of_line(
        &mut self,
        _: &DeleteToEndOfLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.core.selected_range.is_empty() {
            self.replace_text_in_range(None, "", window, cx);
            self.pause_blink_cursor(cx);
            return;
        }

        let mut offset = self.end_of_line();
        if offset == self.cursor() {
            offset = (offset + 1).clamp(0, self.core.text.len());
        }
        self.replace_text_in_range_silent(
            Some(self.range_to_utf16(&(self.cursor()..offset))),
            "",
            window,
            cx,
        );
        self.pause_blink_cursor(cx);
    }

    pub(super) fn delete_previous_word(
        &mut self,
        _: &DeleteToPreviousWordStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.core.selected_range.is_empty() {
            self.replace_text_in_range(None, "", window, cx);
            self.pause_blink_cursor(cx);
            return;
        }

        let offset = self.previous_start_of_word();
        self.replace_text_in_range_silent(
            Some(self.range_to_utf16(&(offset..self.cursor()))),
            "",
            window,
            cx,
        );
        self.pause_blink_cursor(cx);
    }

    pub(super) fn delete_next_word(
        &mut self,
        _: &DeleteToNextWordEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.core.selected_range.is_empty() {
            self.replace_text_in_range(None, "", window, cx);
            self.pause_blink_cursor(cx);
            return;
        }

        let offset = self.next_end_of_word();
        self.replace_text_in_range_silent(
            Some(self.range_to_utf16(&(self.cursor()..offset))),
            "",
            window,
            cx,
        );
        self.pause_blink_cursor(cx);
    }

    pub(super) fn enter(&mut self, action: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        // 多行模式且启用 `submit_on_enter` 时，普通 `Enter`（不带 Shift）被视为
        // 提交：传播动作并发送 PressEnter，不插入换行。`Shift+Enter` 仍插入换行。
        let insert_newline = self.mode.is_multi_line() && (!self.submit_on_enter || action.shift);

        // 只读时不换行，交由外部处理。
        if insert_newline && self.read_only {
            cx.propagate();
            return;
        }

        // 多光标：每处换行并延续缩进（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if insert_newline && !self.extra_selections.is_empty() {
                self.enter_cursors(window, cx);
                // 换行后打断撤销分组：下一次输入自成单元，一次 undo 停在行边界。
                self.core.history.break_group();
                return;
            }
        }

        if insert_newline {
            // 获取当前行缩进
            let indent = if self.mode.is_multi_line() {
                self.indent_of_next_line()
            } else {
                "".to_string()
            };

            // 电缩进：`{` 后回车多缩一级；若光标后紧跟 `}` 则拆成三行。
            let cursor = self.cursor();
            let (new_line_text, middle) = self.electric_newline(&indent, cursor);
            self.replace_text_in_range_silent(None, &new_line_text, window, cx);
            if let Some(at) = middle {
                self.core.selected_range = (at..at).into();
                self.core.selection_reversed = false;
            }
            self.pause_blink_cursor(cx);
            // 换行后打断撤销分组：下一次输入自成单元，一次 undo 停在行边界。
            self.core.history.break_group();
        } else {
            // 单行输入或提交式回车：仅发送事件（例如对话框确认、聊天发送）。
            cx.propagate();
        }

        cx.emit(InputEvent::PressEnter {
            secondary: action.secondary,
            shift: action.shift,
        });
    }

    pub(super) fn clean(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 用户触发的清空（清除按钮/Esc），只读时不执行。
        if self.read_only {
            return;
        }
        self.replace_text("", window, cx);
        self.core.selected_range = (0..0).into();
        self.scroll_to(0, None, cx);
    }

    pub(crate) fn escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if self.core.ime_marked_range.is_some() {
            self.unmark_text(window, cx);
        }

        // 多光标优先坍缩（VS Code 行为），再走清空/传播。
        if !self.extra_selections.is_empty() {
            self.extra_selections.clear();
            cx.notify();
            return;
        }
        if self.clean_on_escape {
            return self.clean(window, cx);
        }

        cx.propagate();
    }

    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 若存在 IME 标记范围且为空（意味着按 Esc 中止了 IME 输入），
        // 清除该标记范围。
        if let Some(ime_marked_range) = &self.core.ime_marked_range {
            if ime_marked_range.len() == 0 {
                self.core.ime_marked_range = None;
            }
        }

        self.selecting = true;
        let offset = self.index_for_mouse_position(event.position);

        // Alt+左键：加一个多光标（仅多行可编辑输入，不启动拖选；
        // `editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if event.button == MouseButton::Left
                && event.modifiers.alt
                && self.mode.is_multi_line()
                && !self.disabled
                && !self.read_only
            {
                self.selecting = false;
                let at = offset.min(self.core.text.len());
                self.extra_selections.push(Selection::new(at, at));
                self.normalize_extras();
                cx.notify();
                return;
            }
        }

        // 三击选中一行
        if event.button == MouseButton::Left && event.click_count >= 3 {
            self.select_line(offset, window, cx);
            return;
        }

        // 双击选中一个单词
        if event.button == MouseButton::Left && event.click_count == 2 {
            self.select_word(offset, window, cx);
            return;
        }

        // 鼠标右键：将光标移动到该位置（先坍缩多光标；`editor` 门控）。
        if event.button == MouseButton::Right {
            #[cfg(feature = "editor")]
            self.clear_extra_cursors(cx);
            if !self.core.selected_range.contains(offset) {
                self.move_to(offset, None, cx);
            }
            return;
        }

        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, None, cx)
        }
    }

    pub(super) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if event.button == MouseButton::Right {
            return;
        }
        if self.core.selected_range.is_empty() {
            self.core.selection_reversed = false;
        }
        self.selecting = false;
        self.selected_word_range = None;
        self.auto_scroll.stop();
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // 检查鼠标是否在边界内
        let within_bounds = self
            .last_bounds
            .as_ref()
            .map(|bounds| bounds.contains(&event.position))
            .unwrap_or(false);

        if !within_bounds {
            return;
        }
    }

    pub(super) fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line_height = self
            .last_layout
            .as_ref()
            .map(|layout| layout.line_height)
            .unwrap_or(window.line_height());
        let delta = event.delta.pixel_delta(line_height);

        let old_offset = self.scroll_handle.offset();
        self.update_scroll_offset(Some(old_offset + delta), cx);

        // 仅当偏移量实际变化时才停止传播
        if self.scroll_handle.offset() != old_offset {
            cx.stop_propagation();
        }
    }

    pub(super) fn update_scroll_offset(
        &mut self,
        offset: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        let mut offset = offset.unwrap_or(self.scroll_handle.offset());
        // 除左对齐外，光标右侧预留位置
        let safe_x_offset = if self.text_align == TextAlign::Left {
            px(0.)
        } else {
            -CURSOR_WIDTH
        };

        let safe_y_range =
            (-self.scroll_size.height + self.input_bounds.size.height).min(px(0.0))..px(0.);
        let safe_x_range = (-self.scroll_size.width + self.input_bounds.size.width + safe_x_offset)
            .min(safe_x_offset)..px(0.);

        offset.y = if self.mode.is_single_line() {
            px(0.)
        } else {
            offset.y.clamp(safe_y_range.start, safe_y_range.end)
        };
        offset.x = offset.x.clamp(safe_x_range.start, safe_x_range.end);
        self.scroll_handle.set_offset(offset);
        cx.notify();
    }

    /// 滚动使给定偏移量可见。
    ///
    /// 若 `direction` 为 Some，则保持边缘在同一侧。
    pub(crate) fn scroll_to(
        &mut self,
        offset: usize,
        direction: Option<MoveDirection>,
        cx: &mut Context<Self>,
    ) {
        let Some(last_layout) = self.last_layout.as_ref() else {
            return;
        };
        let Some(bounds) = self.last_bounds.as_ref() else {
            return;
        };

        let mut scroll_offset = self.scroll_handle.offset();
        let was_offset = scroll_offset;
        let line_height = last_layout.line_height;

        let point = self.core.text.offset_to_point(offset);

        let row = point.row;

        // 通过将前面的行数乘以行高计算行偏移
        let mut row_offset_y = line_height * self.display_map.buffer_line_to_display_row(row);

        // 右对齐使用 0 边距：光标指示器在 layout_cursor 中被钳制在边界内，
        // 因此在此处移动文本会导致首次点击的视觉跳跃。
        let safety_margin = match last_layout.text_align {
            TextAlign::Left => RIGHT_MARGIN,
            TextAlign::Right => px(0.),
            TextAlign::Center => CURSOR_WIDTH,
        };
        if let Some(line) = last_layout
            .lines
            .get(row.saturating_sub(last_layout.visible_range.start))
        {
            // 检查水平滚动与软换行
            if let Some(pos) = line.position_for_index(point.column, last_layout, false) {
                let bounds_width = bounds.size.width - last_layout.line_number_width;
                let col_offset_x = pos.x;
                row_offset_y += pos.y;
                if col_offset_x - safety_margin < -scroll_offset.x {
                    // 位置超出可见区域，滚动使其可见
                    scroll_offset.x = -col_offset_x + safety_margin;
                } else if col_offset_x + safety_margin > -scroll_offset.x + bounds_width {
                    scroll_offset.x = -(col_offset_x - bounds_width + safety_margin);
                }
            }
        }

        // 将行滚入视图。使用与 `TextElement::layout_cursor` 相同的边缘间距助手，
        // 使滚动入视图的两条路径一致。
        let edge_height = if direction.is_some() && self.mode.has_editor_chrome() {
            super::element::cursor_surrounding_padding(
                self.mode.is_auto_grow(),
                self.cursor_surrounding_lines,
                last_layout.visible_range.len(),
                line_height,
            )
        } else {
            line_height
        };
        if row_offset_y - edge_height + line_height < -scroll_offset.y {
            // 向上滚动
            scroll_offset.y = -row_offset_y + edge_height - line_height;
        } else if row_offset_y + edge_height > -scroll_offset.y + bounds.size.height {
            // 向下滚动
            scroll_offset.y = -(row_offset_y - bounds.size.height + edge_height);
        }

        // 已处于正确位置时避免不必要的滚动。
        if direction == Some(MoveDirection::Up) {
            scroll_offset.y = scroll_offset.y.max(was_offset.y);
        } else if direction == Some(MoveDirection::Down) {
            scroll_offset.y = scroll_offset.y.min(was_offset.y);
        }

        // 将延迟目标钳制到与 `update_scroll_offset` 一致的合法范围，
        // 避免绘制时在钳制前出现过度滚动帧。
        let safe_y_min = (-self.scroll_size.height + self.input_bounds.size.height).min(px(0.));
        scroll_offset.x = scroll_offset.x.min(px(0.));
        scroll_offset.y = scroll_offset.y.clamp(safe_y_min, px(0.));
        self.deferred_scroll_offset = Some(scroll_offset);
        cx.notify();
    }

    /// 滚动使给定字节偏移可见，不改变光标与选区。
    ///
    /// 首次布局前调用无效果（内部尚无 layout 时直接返回）。
    /// 只想滚动、不想碰选区时用它；设置选区请用 [`Self::set_selected_range`]。
    pub fn reveal_offset(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.scroll_to(offset, None, cx);
    }

    /// 滚动使给定区间可见（以区间起点为准），不改变光标与选区。
    ///
    /// 语义同 [`Self::reveal_offset`]。
    pub fn reveal_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        self.reveal_offset(range.start, cx);
    }

    #[cfg(feature = "editor")]
    /// 获取文档符号大纲（需高亮器实现 `document_symbols`，如 tree-sitter 后端）。
    ///
    /// 未设置高亮器或后端不支持时返回空。范围为全文 UTF-8 字节偏移，
    /// 可直接用于 [`Self::goto_symbol`]。
    pub fn document_symbols(&self) -> Vec<crate::highlight::DocumentSymbol> {
        match self.highlighter.as_ref() {
            Some(highlighter) => highlighter.document_symbols(&self.core.text),
            None => Vec::new(),
        }
    }

    #[cfg(feature = "editor")]
    /// 跳转到文档符号：光标落符号起始处并滚动可见。
    pub fn goto_symbol(
        &mut self,
        symbol: &crate::highlight::DocumentSymbol,
        cx: &mut Context<Self>,
    ) {
        let offset = symbol.range.start.min(self.core.text.len());
        self.set_selected_range(offset..offset, cx);
        self.reveal_offset(offset, cx);
    }

    #[cfg(feature = "editor")]
    /// 设置语法高亮器（如 `highlight::rust_highlighter()`，需 `--features tree-sitter`）。
    ///
    /// 设置后立即对全文刷新高亮装饰与折叠候选；后续编辑自动刷新。
    /// 传入 `None` 清除高亮（同时清空高亮装饰集合与折叠候选）。
    pub fn set_highlighter(
        &mut self,
        highlighter: Option<Box<dyn Highlighter>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.highlighter = highlighter;
        if self.highlighter.is_some() {
            self.refresh_highlight(window, cx);
        } else {
            // 原地清空（`collection.clear` 走 entity.update，借用中调用会重入 panic）。
            if let Some(collection) = self.highlight_collection.take() {
                collection.set_in_place(&mut self.core.decorations, Vec::new());
            }
            self.display_map.set_fold_candidates(Vec::new());
            cx.notify();
        }
    }

    #[cfg(feature = "editor")]
    /// 刷新语法高亮装饰与折叠候选（有 highlighter 时；编辑后调用）。
    fn refresh_highlight(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(highlighter) = self.highlighter.as_mut() else {
            return;
        };
        let folding = self.mode.is_folding();
        highlighter.update(None, &self.core.text, folding, window, cx);
        let resolver = ThemeHighlightResolver::from_app(cx);
        let len = self.core.text.len();
        let decorations: Vec<TextDecoration> = highlighter
            .styles(&(0..len), &resolver)
            .into_iter()
            .map(|(range, style)| TextDecoration::new(range, style))
            .collect();
        let folds = highlighter
            .fold_ranges(&self.core.text)
            .into_iter()
            .map(|range| FoldRange::new(range.start, range.end))
            .collect();
        // 原地写入（`collection.set` 走 entity.update，此处已在借用中，会重入 panic）。
        // normalize 兜底：stale 范围/合成中间态的错位边界在此吸附到字符边界，
        // 否则布局按字节切分 runs 会 panic。
        if let Some(collection) = self.highlight_collection.clone() {
            let decorations = normalize(&self.core.text, decorations);
            if collection.set_in_place(&mut self.core.decorations, decorations) {
                cx.notify();
            }
        } else {
            self.highlight_collection = Some(self.create_decorations_collection(decorations, cx));
        }
        self.display_map.set_fold_candidates(folds);
    }

    pub(super) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        // 多光标：各选区换行拼接（全塌缩时复制各光标所在整行；`editor` 门控）。
        #[cfg(feature = "editor")]
        {
            if !self.extra_selections.is_empty() {
                return self.copy_cursors(cx);
            }
        }
        if self.core.selected_range.is_empty() {
            return;
        }

        let selected_text = self.core.text.slice(self.core.selected_range).to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
    }

    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only || self.core.selected_range.is_empty() && self.extra_selections.is_empty()
        {
            return;
        }
        // 多光标：复制后删除所有光标范围（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if !self.extra_selections.is_empty() {
                return self.cut_cursors(window, cx);
            }
        }

        let selected_text = self.core.text.slice(self.core.selected_range).to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected_text));

        self.replace_text_in_range_silent(None, "", window, cx);
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        // 多光标：每处贴全文（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if !self.extra_selections.is_empty() {
                if let Some(clipboard) = cx.read_from_clipboard() {
                    let new_text = clipboard.text().unwrap_or_default();
                    self.multi_insert(&new_text, window, cx);
                }
                return;
            }
        }
        if let Some(clipboard) = cx.read_from_clipboard() {
            let new_text = clipboard.text().unwrap_or_default();
            self.replace_text_in_range_silent(None, &new_text, window, cx);
            self.scroll_to(self.cursor(), None, cx);
        }
    }

    pub(crate) fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.core.history.ignore = true;
        if let Some(changes) = self.core.history.undo() {
            for change in changes {
                let range_utf16 = self.range_to_utf16(&change.new_range.into());
                self.replace_text_in_range_silent(Some(range_utf16), &change.old_text, window, cx);
            }
        }
        self.core.history.ignore = false;
    }

    pub(super) fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.core.history.ignore = true;
        if let Some(changes) = self.core.history.redo() {
            for change in changes {
                let range_utf16 = self.range_to_utf16(&change.old_range.into());
                self.replace_text_in_range_silent(Some(range_utf16), &change.new_text, window, cx);
            }
        }
        self.core.history.ignore = false;
    }

    /// 获取光标的字节偏移。
    ///
    /// 该偏移量为 UTF-8 偏移。
    pub fn cursor(&self) -> usize {
        self.core.cursor()
    }

    /// 上次布局视口中的可见行范围，首次布局前为 `None`。
    pub fn visible_row_range(&self) -> Option<std::ops::Range<usize>> {
        self.last_layout.as_ref().map(|l| l.visible_range.clone())
    }

    /// 当前编辑视口的滚动偏移。
    pub fn scroll_offset(&self) -> crate::Point<crate::Pixels> {
        self.scroll_handle.offset()
    }

    /// 设置编辑视口的滚动偏移。
    ///
    /// 偏移量会被钳制到合法范围，并在下次布局后应用。
    pub fn set_scroll_offset(
        &mut self,
        offset: crate::Point<crate::Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.deferred_scroll_offset = Some(offset);
        cx.notify();
    }

    /// 已布局的行高；首次布局前为 `None`。
    pub fn line_height(&self) -> Option<crate::Pixels> {
        self.last_layout.as_ref().map(|l| l.line_height)
    }

    /// 返回当前选择，作为文本的字节范围。
    ///
    /// 未选择文本时范围为空（`start == end`）；此时偏移量等于 `cursor()`。
    /// 字节偏移以底层 rope 的字节单位计。
    pub fn selected_range(&self) -> std::ops::Range<usize> {
        self.core.selected_range.into()
    }

    /// 使用 UTF-8 字节偏移设置选择范围。
    ///
    /// 设置选区会经 `move_to` → `scroll_to` 自动把光标滚动到可见，无需手动调滚动；
    /// 只想滚动、不想改选区时请用 [`Self::reveal_offset`] / [`Self::reveal_range`]。
    pub fn set_selected_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let len = self.core.text.len();
        let start = range.start.min(len);
        let end = range.end.min(len);

        self.move_to(start, None, cx);
        self.core.selection_reversed = false;
        self.selected_word_range = None;
        self.select_to(end, cx);
    }

    pub(crate) fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        // 文本为空时总是返回 0
        if self.core.text.len() == 0 {
            return 0;
        }

        let (Some(bounds), Some(last_layout)) =
            (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };

        let line_height = last_layout.line_height;
        let line_number_width = last_layout.line_number_width;

        // TIP: 关于 IBeam 光标
        //
        // 若光标样式为 IBeam，鼠标位置位于光标中间（这是操作系统的特殊行为）。

        // 位置相对于文本框边界。
        //
        // bounds.origin：
        //
        // - 包含输入内边距。
        // - 包含滚动偏移。
        let inner_position = position - bounds.origin - point(line_number_width, px(0.));

        let mut y_offset = last_layout.visible_top;

        // 遍历可见缓冲行（紧凑，无隐藏条目）
        for (vi, (line_layout, _buffer_line)) in last_layout
            .lines
            .iter()
            .zip(last_layout.visible_buffer_lines.iter())
            .enumerate()
        {
            let line_start_offset = last_layout.visible_line_byte_offsets[vi];

            // 计算该显示行的行原点
            let line_origin = point(px(0.), y_offset);
            let pos = inner_position - line_origin;

            // 单行模式下使用 closest_index_for_x 返回偏移
            if self.mode.is_single_line() {
                let local_index = line_layout.closest_index_for_x(pos.x, last_layout);
                let index = line_start_offset + local_index;
                return if self.masked {
                    self.core
                        .text
                        .char_index_to_offset(index / MASK_CHAR.len_utf8())
                } else {
                    index.min(self.core.text.len())
                };
            }

            // 检查鼠标是否在该行的边界内
            if let Some(local_index) = line_layout.closest_index_for_position(pos, last_layout) {
                let index = line_start_offset + local_index;
                return if self.masked {
                    self.core
                        .text
                        .char_index_to_offset(index / MASK_CHAR.len_utf8())
                } else {
                    index.min(self.core.text.len())
                };
            } else if pos.y < px(0.) {
                // 鼠标在该行上方，返回该行起始
                return if self.masked {
                    self.core
                        .text
                        .char_index_to_offset(line_start_offset / MASK_CHAR.len_utf8())
                } else {
                    line_start_offset
                };
            }

            y_offset += line_layout.size(line_height).height;
        }

        // 鼠标在所有可见行下方，返回文本末尾
        self.core.text.len()
    }

    /// 从当前光标位置选择到给定偏移量。
    ///
    /// 偏移量为 UTF-8 偏移。
    ///
    /// 确保使用 self.next_boundary 或 self.previous_boundary 获得正确的偏移量。
    pub(crate) fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.clamp(0, self.core.text.len());
        if self.core.selection_reversed {
            self.core.selected_range.start = offset
        } else {
            self.core.selected_range.end = offset
        };

        if self.core.selected_range.end < self.core.selected_range.start {
            self.core.selection_reversed = !self.core.selection_reversed;
            self.core.selected_range =
                (self.core.selected_range.end..self.core.selected_range.start).into();
        }

        // 确保保持单词选择范围
        if let Some(word_range) = self.selected_word_range.as_ref() {
            if self.core.selected_range.start > word_range.start {
                self.core.selected_range.start = word_range.start;
            }
            if self.core.selected_range.end < word_range.end {
                self.core.selected_range.end = word_range.end;
            }
        }
        if self.core.selected_range.is_empty() {
            self.update_preferred_column();
        }
        #[cfg(feature = "editor")]
        self.refresh_bracket_match(cx);
        #[cfg(feature = "editor")]
        self.refresh_current_line(cx);
        cx.notify()
    }

    /// 取消当前选择的文本。
    pub fn unselect(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.cursor();
        self.core.selected_range = (offset..offset).into();
        cx.notify()
    }

    #[inline]
    pub(crate) fn offset_from_utf16(&self, offset: usize) -> usize {
        self.core.text.offset_utf16_to_offset(offset)
    }

    #[inline]
    pub(crate) fn offset_to_utf16(&self, offset: usize) -> usize {
        self.core.text.offset_to_offset_utf16(offset)
    }

    #[inline]
    pub(crate) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    #[inline]
    pub(super) fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    /// 若偏移量落在隐藏（折叠）行上，向后钳制到折叠头行的末尾
    /// （折叠前最后一个可见位置）。
    fn clamp_offset_to_visible_backward(&self, offset: usize) -> usize {
        let line = self.core.text.offset_to_point(offset).row;
        if self.display_map.is_buffer_line_hidden(line) {
            for fold in self.display_map.folded_ranges() {
                if line > fold.start_line && line <= fold.end_line {
                    return self.core.text.line_end_offset(fold.start_line);
                }
            }
        }
        offset
    }

    /// 若偏移量落在隐藏（折叠）行上，向前钳制到折叠结束行的起点
    /// （折叠后第一个可见位置）。
    fn clamp_offset_to_visible_forward(&self, offset: usize) -> usize {
        let line = self.core.text.offset_to_point(offset).row;
        if self.display_map.is_buffer_line_hidden(line) {
            for fold in self.display_map.folded_ranges() {
                if line > fold.start_line && line <= fold.end_line {
                    return self.core.text.line_start_offset(fold.end_line);
                }
            }
        }
        offset
    }

    pub(crate) fn previous_boundary(&self, offset: usize) -> usize {
        let mut offset = self
            .core
            .text
            .clip_offset(offset.saturating_sub(1), Bias::Left);
        if let Some(ch) = self.core.text.char_at(offset) {
            if ch == '\r' {
                offset -= 1;
            }
        }

        self.clamp_offset_to_visible_backward(offset)
    }

    pub(crate) fn next_boundary(&self, offset: usize) -> usize {
        let mut offset = self.core.text.clip_offset(offset + 1, Bias::Right);
        if let Some(ch) = self.core.text.char_at(offset) {
            if ch == '\r' {
                offset += 1;
            }
        }

        self.clamp_offset_to_visible_forward(offset)
    }

    /// 返回是否渲染光标：输入框聚焦且当前闪烁光标可见。
    pub(crate) fn show_cursor(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle.is_focused(window)
            && !self.disabled
            && self.blink_cursor.read(cx).visible()
            && window.is_window_active()
    }

    /// 返回输入框当前是否聚焦。
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    fn on_focus(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| {
            cursor.start(cx);
        });
        cx.emit(InputEvent::Focus);
    }

    fn on_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 注意：失焦时不取消选择。
        // 因为用户可能想通过 AppMenuBar 复制选中的文本（会获取焦点句柄）。

        self.blink_cursor.update(cx, |cursor, cx| {
            cursor.stop(cx);
        });
        self.clamp_number_value(window, cx);
        cx.emit(InputEvent::Blur);
        cx.notify();
    }

    /// 将数值钳制到 `min`/`max` 范围，用于失焦时。
    ///
    /// 输入时允许越界值（例如 min 为 10 时 `1` 是 `15` 的中间状态），
    /// 失焦时钳制。
    fn clamp_number_value(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.mode.is_single_line() {
            return;
        }
        if !matches!(self.mask_pattern, MaskPattern::Number { .. }) {
            return;
        }
        if self.number_min.is_none() && self.number_max.is_none() {
            return;
        }

        let Ok(value) = self.unmask_value().parse::<f64>() else {
            return;
        };

        let clamped = match (self.number_min, self.number_max) {
            (Some(min), _) if value < min => min,
            (_, Some(max)) if value > max => max,
            _ => return,
        };

        // 钳制后的值必须通过 `pattern`/`validate` 检查，否则保持原值。
        let new_text = clamped.to_string();
        if !self.is_valid_input(&new_text, cx) {
            return;
        }

        let range = self.range_to_utf16(&(0..self.core.text.len()));
        self.replace_text_in_range_silent(Some(range), &new_text, window, cx);
    }

    pub(crate) fn pause_blink_cursor(&mut self, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |cursor, cx| {
            cursor.pause(cx);
        });
    }

    pub(super) fn on_key_down(&mut self, _: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_blink_cursor(cx);
    }

    pub(super) fn on_drag_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.core.text.len() == 0 {
            return;
        }

        if self.last_layout.is_none() {
            return;
        }

        if !self.focus_handle.is_focused(window) {
            return;
        }

        if !self.selecting {
            return;
        }

        self.auto_scroll.last_drag_position = Some(event.position);
        let offset = self.index_for_mouse_position(event.position);
        self.select_to(offset, cx);

        if !self.mode.is_single_line() {
            // 用 CSS 内边距扩展 input_bounds，使边界反映完整的可见元素。
            // 否则内边距区域（视觉上在输入框内部）的鼠标位置会看起来在
            // 边界之外，触发最大速度。
            let pad = self.editor_scrollbar_paddings.get();
            let scroll_bounds = crate::Bounds::new(
                point(
                    self.input_bounds.origin.x - pad.left,
                    self.input_bounds.origin.y - pad.top,
                ),
                crate::size(
                    self.input_bounds.size.width + pad.left + pad.right,
                    self.input_bounds.size.height + pad.top + pad.bottom,
                ),
            );
            let delta = AutoScroll::compute_delta(event.position.y, scroll_bounds);
            // 输入的 ScrollHandle 使用负 y 向下；取反正的向下增量。
            let scroll_delta = delta.map(|d| -d);
            self.auto_scroll.set(scroll_delta, cx, |delta, state, cx| {
                let current = state.scroll_handle.offset();
                state.update_scroll_offset(Some(point(current.x, current.y + delta)), cx);
                if let Some(pos) = state.auto_scroll.last_drag_position {
                    let offset = state.index_for_mouse_position(pos);
                    state.select_to(offset, cx);
                }
            });
        }
    }

    /// 在应用到输入框前规范化插入的文本。
    ///
    /// 对数字输入（使用 [`MaskPattern::Number`]）将全角数字字符转换为对应的
    /// ASCII 等价形式，例如 `12。5` -> `12.5`。
    fn normalize_input<'a>(&self, new_text: &'a str) -> Cow<'a, str> {
        let normalized = if matches!(self.mask_pattern, MaskPattern::Number { .. }) {
            normalize_number_input(new_text)
        } else {
            Cow::Borrowed(new_text)
        };

        if self.mode.is_single_line() && normalized.contains(['\n', '\r']) {
            Cow::Owned(normalized.replace(['\n', '\r'], ""))
        } else {
            normalized
        }
    }

    pub(super) fn is_valid_input(&self, new_text: &str, cx: &mut Context<Self>) -> bool {
        if new_text.is_empty() {
            return true;
        }

        if let Some(validate) = &self.validate {
            if !validate(new_text, cx) {
                return false;
            }
        }

        if !self.mask_pattern.is_valid(new_text) {
            return false;
        }

        let Some(pattern) = &self.pattern else {
            return true;
        };

        pattern.is_match(new_text)
    }

    /// 设置用于格式化输入文本的掩码模式。
    ///
    /// 模式可包含：
    /// - 9: 任意数字或点
    /// - A: 任意字母
    /// - *: 任意字符
    /// - 其他字符视为字面掩码字符
    ///
    /// 示例：电话号码 "(999)999-999"
    pub fn mask_pattern(mut self, pattern: impl Into<MaskPattern>) -> Self {
        self.mask_pattern = pattern.into();
        self.mask_pattern_set = true;
        if let Some(placeholder) = self.mask_pattern.placeholder() {
            self.placeholder = placeholder.into();
        }
        self
    }

    /// 设置输入框的掩码模式（链式构建）。
    ///
    /// 与 [`Self::set_mask_pattern`] 等价，便于在构建时链式调用。
    pub fn set_mask_pattern(
        &mut self,
        pattern: impl Into<MaskPattern>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mask_pattern = pattern.into();
        self.mask_pattern_set = true;
        if let Some(placeholder) = self.mask_pattern.placeholder() {
            self.placeholder = placeholder.into();
        }
        cx.notify();
    }

    pub(super) fn set_input_bounds(&mut self, new_bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        let wrap_width_changed = self.input_bounds.size.width != new_bounds.size.width;
        self.input_bounds = new_bounds;

        // 宽度变化时更新 display_map 的 wrap_width。
        if let Some(last_layout) = self.last_layout.as_ref() {
            if wrap_width_changed {
                let wrap_width = if !self.soft_wrap {
                    // None 禁用换行（将使用 Pixels::MAX）
                    None
                } else {
                    last_layout.wrap_width
                };

                self.display_map.on_layout_changed(wrap_width, cx);
                self.mode.update_auto_grow(&self.display_map);
                cx.notify();
            }
        }
    }

    pub(super) fn selected_text(&self) -> RopeSlice<'_> {
        let range_utf16 = self.range_to_utf16(&self.core.selected_range.into());
        let range = self.range_from_utf16(&range_utf16);
        self.core.text.slice(range)
    }

    /// 返回当前输入内容中给定 UTF-8 字节范围的渲染边界。
    ///
    /// 当请求的范围当前未布局或不可见时返回 `None`。
    pub fn range_to_bounds(&self, range: &Range<usize>) -> Option<Bounds<Pixels>> {
        let Some(last_layout) = self.last_layout.as_ref() else {
            return None;
        };

        let Some(last_bounds) = self.last_bounds else {
            return None;
        };

        let (_, _, start_pos) = self.line_and_position_for_offset(range.start);
        let (_, _, end_pos) = self.line_and_position_for_offset(range.end);

        let Some(start_pos) = start_pos else {
            return None;
        };
        let Some(end_pos) = end_pos else {
            return None;
        };

        Some(Bounds::from_corners(
            last_bounds.origin + start_pos,
            last_bounds.origin + end_pos + point(px(0.), last_layout.line_height),
        ))
    }

    /// 静默替换范围内文本。
    ///
    /// 不会触发任何 UI 交互（例如自动补全）。
    pub(crate) fn replace_text_in_range_silent(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range_raw(range_utf16, new_text, window, cx);
    }

    /// 替换范围内文本的原始实现（无只读拦截、无自动闭合）。
    ///
    /// 程序化写入（`insert`/`replace`/`set_value`/撤销/行操作等）与内部
    /// 调用走这里；用户键入走 trait 方法 [`EntityInputHandler::replace_text_in_range`]。
    pub(crate) fn replace_text_in_range_raw(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // `window` 仅供高亮刷新（`editor`）；关闭时显式丢弃以免 unused 警告。
        #[cfg(not(feature = "editor"))]
        let _ = window;
        if self.disabled {
            return;
        }

        if self.blink_cursor.read(cx).visible() {
            self.pause_blink_cursor(cx);
        }

        // 注意：规范化保持 UTF-16 长度，但可能改变 UTF-8 字节长度，
        // 因此下面的所有字节偏移计算必须使用规范化后的文本。
        let new_text = self.normalize_input(new_text);
        let new_text: &str = &new_text;

        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.core.ime_marked_range.map(|range| {
                let range = self.range_to_utf16(&(range.start..range.end));
                self.range_from_utf16(&range)
            }))
            .unwrap_or(self.core.selected_range.into());

        let old_text = self.core.text.clone();

        // 单行校验与掩码需要替换后文本：在克隆上试算，不提交
        //（原来是提交后回滚；等价，且无中间状态）。
        let mut masked: Option<(SharedString, usize)> = None;
        if self.mode.is_single_line() {
            let mut trial = old_text.clone();
            trial.replace(range.clone(), new_text);
            let pending_text = trial.to_string();
            // 检查新文本是否合法。
            //
            // 仅在旧文本合法时拒绝该编辑，以避免陷入预先存在的非法文本
            // （例如不符合规则的 `default_value`），用户仍可编辑以修复它。
            if !self.is_valid_input(&pending_text, cx)
                && self.is_valid_input(&old_text.to_string(), cx)
            {
                return;
            }

            if !self.mask_pattern.is_none() {
                let mask_text = self.mask_pattern.mask(&pending_text);
                // 掩码是否改变了文本，例如重组分隔符或补全前导点。
                if mask_text.as_str() != pending_text {
                    masked = Some((mask_text, pending_text.len()));
                }
            }
        }

        let new_offset = if let Some((mask_text, pending_len)) = masked {
            // 掩码改写路径（表单专用）：提交替换→重写掩码文本→清空装饰→
            // 整文档历史（基于段的历史条目不再匹配掩码后文档）。
            self.core.text.replace(range.clone(), new_text);
            self.core.text = Rope::from(mask_text.as_str());
            self.core.decorations.clear();
            let final_text = self.core.text.to_string();
            self.core
                .push_history(&old_text, &(0..old_text.len()), &final_text);
            self.core.history.end_grouping();
            let new_text_len = (new_text.len() + final_text.len()).saturating_sub(pending_len);
            (range.start + new_text_len).min(final_text.len())
        } else {
            self.core.apply_replace(range.clone(), new_text)
        };
        // 调整折叠后再更新换行映射：移除重叠折叠并移动其他折叠。
        self.display_map
            .adjust_folds_for_edit(&old_text, &range, new_text);
        self.display_map
            .on_text_changed(&self.core.text, &range, &Rope::from(new_text), cx);
        #[cfg(feature = "editor")]
        self.refresh_highlight(window, cx);

        self.core.selected_range = (new_offset..new_offset).into();
        self.core.ime_marked_range.take();
        self.update_preferred_column();
        self.mode.update_auto_grow(&self.display_map);
        #[cfg(feature = "editor")]
        self.refresh_bracket_match(cx);
        #[cfg(feature = "editor")]
        self.refresh_bracket_rainbow(cx);
        #[cfg(feature = "editor")]
        self.refresh_current_line(cx);
        if self.emit_events {
            cx.emit(InputEvent::Change);
        }
        cx.notify();
    }
}

impl EntityInputHandler for InputState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        adjusted_range.replace(self.range_to_utf16(&range));
        Some(self.core.text.slice(range).to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.core.selected_range.into()),
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.core
            .ime_marked_range
            .map(|range| self.range_to_utf16(&range.into()))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.core.ime_marked_range = None;
    }

    /// 替换范围内文本。
    ///
    /// - 若新文本非法，则不替换。
    /// - 若未提供 `range_utf16`，则使用当前选择范围。
    ///
    /// 用户键入入口：只读时直接返回；单个括号/引号走自动闭合。
    /// 程序化写入请走 [`Self::replace_text_in_range_silent`]（直通 raw，不受
    /// 只读与自动闭合影响，与 `disabled` 的处理一致）。
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        // 多光标：原文插入（键入/粘贴都扇出），不走自动闭合
        //（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if range_utf16.is_none()
                && self.core.ime_marked_range.is_none()
                && !self.extra_selections.is_empty()
                && !new_text.is_empty()
            {
                let text = new_text.to_string();
                self.multi_insert(&text, window, cx);
                return;
            }
        }
        // 自动闭合拦截（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        {
            if range_utf16.is_none()
                && self.core.ime_marked_range.is_none()
                && let Some(typed) = super::super::editor::auto_close::single_typed_char(new_text)
                && super::super::editor::auto_close::auto_close_applies(self)
                && super::super::editor::auto_close::handle_typed_char(self, typed, window, cx)
            {
                return;
            }
        }
        self.replace_text_in_range_raw(range_utf16, new_text, window, cx);
    }

    /// 标记文本为 IME 输入的临时插入。
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.read_only {
            return;
        }

        // `window` 仅供高亮刷新（`editor`）；关闭时显式丢弃以免 unused 警告。
        #[cfg(not(feature = "editor"))]
        let _ = window;

        // IME 合成期间坍缩多光标（合成只认主光标；`editor` 门控）。
        #[cfg(feature = "editor")]
        self.clear_extra_cursors(cx);

        // 参见 `replace_text_in_range` 中的相同注释。
        let new_text = self.normalize_input(new_text);
        let new_text: &str = &new_text;

        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.core.ime_marked_range.map(|range| {
                let range = self.range_to_utf16(&(range.start..range.end));
                self.range_from_utf16(&range)
            }))
            .unwrap_or(self.core.selected_range.into());

        let old_text = self.core.text.clone();
        self.core.text.replace(range.clone(), new_text);

        if self.mode.is_single_line() {
            let pending_text = self.core.text.to_string();
            // 参见 `replace_text_in_range` 中的相同注释。
            if !self.is_valid_input(&pending_text, cx)
                && self.is_valid_input(&old_text.to_string(), cx)
            {
                self.core.text = old_text;
                return;
            }
        }

        self.core
            .decorations
            .adjust_for_edit(&range, new_text.len());
        // 调整折叠后再更新换行映射：移除重叠折叠并移动其他折叠。
        self.display_map
            .adjust_folds_for_edit(&old_text, &range, new_text);
        self.display_map
            .on_text_changed(&self.core.text, &range, &Rope::from(new_text), cx);

        if new_text.is_empty() {
            // 取消 IME 输入时取消选择。
            self.core.selected_range = (range.start..range.start).into();
            self.core.ime_marked_range = None;
        } else {
            self.core.ime_marked_range = Some((range.start..range.start + new_text.len()).into());
            self.core.selected_range = new_selected_range_utf16
                .as_ref()
                .map(|range_utf16| {
                    let new_text = Rope::from(new_text);
                    range.start + new_text.offset_utf16_to_offset(range_utf16.start)
                        ..range.start + new_text.offset_utf16_to_offset(range_utf16.end)
                })
                .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len())
                .into();
        }
        self.mode.update_auto_grow(&self.display_map);
        self.core.history.start_grouping();
        self.core.push_history(&old_text, &range, new_text);
        // IME 合成同样改变文本，高亮/折叠候选必须同步刷新，否则 stale 范围
        // 在布局切分 runs 时可能落在多字节字符内部导致 panic
        //（`editor` feature 门控）。
        #[cfg(feature = "editor")]
        self.refresh_highlight(window, cx);
        cx.notify();
    }

    /// 用于定位 IME 候选窗口。
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let line_height = last_layout.line_height;
        let line_number_width = last_layout.line_number_width;
        let range = self.range_from_utf16(&range_utf16);

        let mut start_origin = None;
        let mut end_origin = None;
        let line_number_origin = point(line_number_width, px(0.));
        let mut y_offset = last_layout.visible_top;

        for (vi, line) in last_layout.lines.iter().enumerate() {
            if start_origin.is_some() && end_origin.is_some() {
                break;
            }

            let index_offset = last_layout.visible_line_byte_offsets[vi];

            if start_origin.is_none() {
                if let Some(p) = line.position_for_index(
                    range.start.saturating_sub(index_offset),
                    last_layout,
                    false,
                ) {
                    start_origin = Some(p + point(px(0.), y_offset));
                }
            }

            if end_origin.is_none() {
                if let Some(p) = line.position_for_index(
                    range.end.saturating_sub(index_offset),
                    last_layout,
                    false,
                ) {
                    end_origin = Some(p + point(px(0.), y_offset));
                }
            }

            y_offset += line.size(line_height).height;
        }

        let start_origin = start_origin.unwrap_or_default();
        let mut end_origin = end_origin.unwrap_or_default();
        // 确保在同一行。
        end_origin.y = start_origin.y;

        Some(Bounds::from_corners(
            bounds.origin + line_number_origin + start_origin,
            // + line_height 用于在光标行下方显示 IME 面板。
            bounds.origin + line_number_origin + point(end_origin.x, end_origin.y + line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: crate::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let last_layout = self.last_layout.as_ref()?;
        let line_point = self.last_bounds?.localize(&point)?;

        for (vi, line) in last_layout.lines.iter().enumerate() {
            let offset = last_layout.visible_line_byte_offsets[vi];
            if let Some(utf8_index) = line.index_for_position(line_point, last_layout) {
                return Some(self.offset_to_utf16(offset + utf8_index));
            }
        }

        None
    }
}

impl Focusable for InputState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InputState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self._pending_update {
            self.display_map.ensure_text_prepared(&self.core.text, cx);
            self._pending_update = false;
        }

        let enabled = self.context_menu_enabled;
        let state = cx.entity();
        let el = div()
            .id("input-state")
            .flex_1()
            .when(self.mode.is_multi_line(), |this| this.h_full())
            .flex_grow_1()
            .overflow_x_hidden()
            .child(TextElement::new(cx.entity()).placeholder(self.placeholder.clone()));
        // 右键菜单挂在文本区上（`Input` 外层同样经此实体渲染，故单行/多行全覆盖；
        // 前缀/后缀装饰区不触发）。配置见 `input_ui/context_menu.rs`。
        if enabled {
            el.context_menu(move |menu, window, cx| {
                Self::build_context_menu(menu, &state, window, cx)
            })
            .into_any_element()
        } else {
            el.into_any_element()
        }
    }
}

#[cfg(test)]
mod typing_behavior_tests {
    use super::*;
    use crate::Entity;

    /// 持有输入框状态的测试宿主视图。
    struct Probe {
        state: Entity<InputState>,
    }

    impl crate::Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl crate::IntoElement {
            crate::div()
        }
    }

    #[rgpui::test]
    fn electric_indent_splits_brace_block(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.replace("fn main() {\n}", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 光标放在 `{` 后回车：拆成三行，光标落中间行末（默认缩进 2 空格）。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(11..11, cx));
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.enter(
                    &Enter {
                        secondary: false,
                        shift: false,
                    },
                    window,
                    cx,
                )
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| (state.value().to_string(), state.cursor())),
            ("fn main() {\n  \n}".to_string(), 14)
        );
    }

    #[rgpui::test]
    fn read_only_blocks_typing_but_allows_api(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true).read_only(true));
            state.update(cx, |state, cx| state.replace("hello", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        // 用户键入被拦截（走 trait 方法，与 OS 键入同路径）。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                EntityInputHandler::replace_text_in_range(state, None, "(", window, cx);
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "hello".to_string()
        );
        // 程序化写入不受影响。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.insert(" world", window, cx));
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "hello world".to_string()
        );
        // 光标移动/选择正常。
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_selected_range(0..5, cx));
        });
        assert!(state.read_with(cx, |state, _| state.has_selection()));
    }

    /// 回车打断撤销分组：快输 a、回车、快输 b 全在 1 秒窗内，
    /// 一次 undo 只回退回车后的 b（RustRover 同款行边界行为），而不是整体清空。
    #[rgpui::test]
    fn enter_breaks_undo_group(cx: &mut crate::TestAppContext) {
        cx.update(crate::input_ui::init);
        cx.update(crate::theme::init);
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                EntityInputHandler::replace_text_in_range(state, None, "a", window, cx);
            });
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                state.enter(
                    &Enter {
                        secondary: false,
                        shift: false,
                    },
                    window,
                    cx,
                )
            });
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                EntityInputHandler::replace_text_in_range(state, None, "b", window, cx);
            });
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "a\nb".to_string()
        );
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.undo(&Undo, window, cx));
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "a\n".to_string()
        );
        // 再 undo 回退 a + 换行（回车前的同一输入 burst）。
        cx.update(|window, cx| {
            state.update(cx, |state, cx| state.undo(&Undo, window, cx));
        });
        assert_eq!(
            state.read_with(cx, |state, _| state.value().to_string()),
            "".to_string()
        );
    }
}
