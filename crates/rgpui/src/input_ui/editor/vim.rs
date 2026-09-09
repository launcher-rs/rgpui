//! Vim 模式最小可用（O3，`editor` feature 门控）：normal/insert/visual 三态。
//!
//! 拦截原理（键位即消费）：vim 按键经 keymap 绑定到 [`VimKey`]（`VimNormal` /
//! `VimVisual` / `VimInsert` 上下文），命中即分发 action、不再回放为文本；
//! 未绑定键（如 insert 下字母）照常走文本输入。`Input` 自身上下文里已有绑定
//! 的键（回车/退格/删除/Esc）靠“同节点 tie + 后注册优先”规则被 vim 覆盖
//! （vim 上下文与 `Input` 并入同一 `KeyContext`，见 [`wire_input_context`]）。
//! Esc 走 `capture_key_down`（action 之后跑，只切模式不吞，`Input` 的折叠保留）。
//!
//! # 支持矩阵（v1，打勾即支持）
//!
//! - 模式：normal / insert / visual（含切换 `i a o v Esc`）✅；`:` 命令/宏/
//!   寄存器（系统剪贴板）/插件/可视块 ❌
//! - 移动：`hjkl wb 0 $ gg G` ✅；`fFtT`/`%/`/`{}`/数字前缀计数 ❌
//! - 编辑：`x dd yy p u` + 可视 `y d` ✅；`c/s/r ~/Cc/D/Y/dw` 等 ❌
//! - `dw` 类复合操作（除 `dd`/`yy`/`gg` 外）❌；`d`/`y`/`g` 后跟他键即弃前缀
//! - 数字 `1-9`、`:`、`/`、`.` 在 normal 下无操作（吞掉，不污染文本；计数不做）
//! - `p` 粘贴走内部寄存器（不碰系统剪贴板）；`yy`/`y` 同理
//! - CJK 列对齐按字符数（字节列会有半个字偏差，v1 约束）
//! - 多光标：`move_to` 统一坍缩（纯键盘移动坍缩先例），不扇出

use crate::{Action, App, Context, KeyBinding, SharedString, Window};

use super::super::{InputState, RopeExt as _, Undo};

/// Vim normal 模式上下文（进 `Input` key_context）。
pub(crate) const VIM_NORMAL_CONTEXT: &str = "VimNormal";
/// Vim visual 模式上下文。
pub(crate) const VIM_VISUAL_CONTEXT: &str = "VimVisual";
/// Vim insert 模式上下文（只绑 Esc）。
pub(crate) const VIM_INSERT_CONTEXT: &str = "VimInsert";

/// Vim 按键动作（单动作带键名；`no_json` 与 `Enter` 同例，keymap.json 暂不具名）。
#[derive(Action, Clone, PartialEq, Eq, serde::Deserialize)]
#[action(namespace = vim, no_json)]
pub struct VimKey {
    /// 按键名（绑定名，如 `h`、`G`、`enter`、`escape`）。
    pub key: SharedString,
}

/// Vim 模式（三态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    /// 普通模式（移动/操作；默认）。
    #[default]
    Normal,
    /// 插入模式（键入直通，无 vim 上下文绑定字母键）。
    Insert,
    /// 可视模式（移动扩展选区）。
    Visual,
}

impl VimMode {
    /// 上下文标识（进 `Input` key_context）。
    pub(crate) fn context_id(self) -> &'static str {
        match self {
            VimMode::Normal => VIM_NORMAL_CONTEXT,
            VimMode::Insert => VIM_INSERT_CONTEXT,
            VimMode::Visual => VIM_VISUAL_CONTEXT,
        }
    }

    /// 状态行指示。
    pub fn indicator(self) -> &'static str {
        match self {
            VimMode::Normal => "NORMAL",
            VimMode::Insert => "INSERT",
            VimMode::Visual => "VISUAL",
        }
    }
}

/// Vim 运行时状态（存 `InputState`，随输入走；`EditorState` 只做开关/展示透传）。
#[derive(Debug, Clone)]
pub(crate) struct VimState {
    /// 是否启用（默认关）。
    pub(crate) enabled: bool,
    /// 当前模式。
    pub(crate) mode: VimMode,
    /// 待定前缀（`d`/`y`/`g` 等第二键）。
    pub(crate) pending: Option<char>,
    /// 可视锚点（字节偏移）。
    pub(crate) anchor: Option<usize>,
    /// 上下移动列记忆（字符数）。
    pub(crate) preferred_col: usize,
    /// 内部 yank 寄存器（不碰系统剪贴板，v1 约束）。
    pub(crate) register: String,
    /// 寄存器是否为整行（`yy` 行粘贴，`y` 字符粘贴）。
    pub(crate) register_linewise: bool,
}

impl Default for VimState {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: VimMode::Normal,
            pending: None,
            anchor: None,
            preferred_col: 0,
            register: String::new(),
            register_linewise: false,
        }
    }
}

/// 注册 vim 键位（`input_ui::init` 尾部调用，`editor` 门控；必须在 input 绑定之后，
/// 同节点 tie 靠后注册优先覆盖回车/退格/删除/Esc）。
pub(crate) fn init(cx: &mut App) {
    let mut bindings = Vec::new();
    // normal：移动/操作/模式切换 + 吞键（数字/`:`/`/`/`.`）+ 覆盖键。
    for key in [
        "h",
        "j",
        "k",
        "l",
        "w",
        "b",
        "0",
        "$",
        "G",
        "g",
        "x",
        "i",
        "a",
        "o",
        "v",
        "y",
        "d",
        "p",
        "u",
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "9",
        ":",
        "/",
        ".",
        "enter",
        "escape",
        "backspace",
        "delete",
    ] {
        bindings.push(KeyBinding::new(
            key,
            VimKey { key: key.into() },
            Some(VIM_NORMAL_CONTEXT),
        ));
    }
    // visual：移动扩展 + 操作 + 模式切换（回车/退格/删除走 `Input` 默认：换行/删选区）。
    for key in [
        "h", "j", "k", "l", "w", "b", "0", "$", "G", "y", "d", "x", "v", "escape",
    ] {
        bindings.push(KeyBinding::new(
            key,
            VimKey { key: key.into() },
            Some(VIM_VISUAL_CONTEXT),
        ));
    }
    // insert 不绑字母键（键入直通）；Esc 不绑 keymap（走 `capture_key_down` 单路径，
    // 免得与 `VimKey` 双跑；`Input` 自身 Esc 照跑，只做折叠）。
    cx.bind_keys(bindings);
}

/// 词字符（移动/前缀扫描口径，自动补全同款）。
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 当前行字节区间（起含 `\n`，止不含；返回 `(行首, 行尾)`）。
fn line_range(state: &InputState, row: usize) -> (usize, usize) {
    let text = state.text();
    let start = text.line_start_offset(row);
    let next = text.line_start_offset(row + 1);
    // 末行无下一行：止于全文末；否则止于换行符前。
    let end = if next > start && text.slice(next - 1..next) == "\n" {
        next - 1
    } else {
        next.min(text.len())
    };
    (start, end.min(text.len()))
}

/// 总行数（字节行口径）。
fn line_count(state: &InputState) -> usize {
    let text = state.text();
    text.offset_to_point(text.len()).row + 1
}

/// 行内第 `col` 个字符的字节偏移（字符数口径，超长钳制行尾）。
fn char_offset_in_line(state: &InputState, row: usize, col: usize) -> usize {
    let (start, end) = line_range(state, row);
    let text = state.text();
    let mut offset = start;
    for (count, ch) in text.slice(start..end).chars().enumerate() {
        if count >= col {
            break;
        }
        offset += ch.len_utf8();
    }
    offset
}

/// 光标所在行已走字符数（`preferred_col` 口径）。
fn cursor_char_col(state: &InputState) -> usize {
    let cursor = state.cursor();
    let text = state.text();
    let row = text.offset_to_point(cursor).row;
    let (start, _) = line_range(state, row);
    text.slice(start..cursor.min(text.len())).chars().count()
}

/// 下一字符边界（CJK 安全；行尾钳制由调用方做）。
fn next_boundary(text: &ropey::Rope, cursor: usize) -> usize {
    text.ceil_char_boundary(cursor.saturating_add(1))
        .min(text.len())
}

/// 上一字符边界（CJK 安全）。
fn prev_boundary(text: &ropey::Rope, cursor: usize) -> usize {
    text.floor_char_boundary(cursor.saturating_sub(1))
}

/// 光标移到指定偏移（坍缩选区；列记忆按字符数刷新）。
fn goto(state: &mut InputState, offset: usize, cx: &mut Context<InputState>) {
    let offset = offset.min(state.text().len());
    state.move_to(offset, None, cx);
    state.vim.preferred_col = cursor_char_col(state);
}

/// 上/下移动（列记忆优先，钳制行尾）。
fn move_vertical(state: &mut InputState, delta: isize, cx: &mut Context<InputState>) {
    let cursor = state.cursor();
    let text = state.text();
    let row = text.offset_to_point(cursor).row;
    let target = row
        .saturating_add_signed(delta)
        .min(line_count(state).saturating_sub(1));
    let offset = char_offset_in_line(state, target, state.vim.preferred_col);
    state.move_to(offset, None, cx);
}

/// 下一词首（跨行；无下一词停留）。
fn word_forward(state: &InputState, cursor: usize) -> usize {
    let text = state.text();
    let len = text.len();
    let window = text.slice(cursor.min(len)..).to_string();
    let mut offset = cursor;
    let mut chars = window.chars().peekable();
    // 跳过当前词剩余部分。
    while let Some(&c) = chars.peek() {
        if !is_word_char(c) {
            break;
        }
        offset += c.len_utf8();
        chars.next();
    }
    // 跳过非词（含换行）。
    while let Some(&c) = chars.peek() {
        if is_word_char(c) {
            break;
        }
        offset += c.len_utf8();
        chars.next();
    }
    offset.min(len)
}

/// 上一词首（行首停留；词中回词首）。
fn word_backward(state: &InputState, cursor: usize) -> usize {
    let text = state.text();
    let cursor = cursor.min(text.len());
    // 各字符起始字节表（末项即光标）。
    let mut offsets: Vec<usize> = vec![0];
    for ch in text.slice(..cursor).chars() {
        offsets.push(offsets.last().copied().unwrap_or(0) + ch.len_utf8());
    }
    let chars: Vec<char> = text.slice(..cursor).chars().collect();
    let mut index = chars.len();
    if index > 0 && is_word_char(chars[index - 1]) {
        while index > 0 && is_word_char(chars[index - 1]) {
            index -= 1;
        }
    } else {
        while index > 0 && !is_word_char(chars[index - 1]) {
            index -= 1;
        }
        while index > 0 && is_word_char(chars[index - 1]) {
            index -= 1;
        }
    }
    offsets[index]
}

/// 可视扩展到指定偏移（锚点固定，有序选区）。
fn extend_to(state: &mut InputState, offset: usize, cx: &mut Context<InputState>) {
    let anchor = state.vim.anchor.unwrap_or_else(|| state.cursor());
    let offset = offset.min(state.text().len());
    let (start, end) = if anchor <= offset {
        (anchor, offset)
    } else {
        (offset, anchor)
    };
    state.set_selected_range(start..end, cx);
    state.vim.preferred_col = cursor_char_col(state);
}

/// 切到 normal（坍缩选区到光标；清前缀/锚点）。
fn to_normal(state: &mut InputState, cx: &mut Context<InputState>) {
    let cursor = state.cursor();
    state.vim.mode = VimMode::Normal;
    state.vim.pending = None;
    state.vim.anchor = None;
    state.move_to(cursor, None, cx);
}

/// 切到 insert（不清选区，`a`/`o` 已预移光标）。
fn to_insert(state: &mut InputState, cx: &mut Context<InputState>) {
    state.vim.mode = VimMode::Insert;
    state.vim.pending = None;
    state.vim.anchor = None;
    cx.notify();
}

/// 删除字节区间（选中 + 替换空串，撤销单组）。
fn delete_range(
    state: &mut InputState,
    range: std::ops::Range<usize>,
    window: &mut Window,
    cx: &mut Context<InputState>,
) {
    state.set_selected_range(range, cx);
    state.replace("", window, cx);
}

/// normal 模式单键分发（`true` = 已处理；调用方吞传播）。
fn normal_key(
    state: &mut InputState,
    key: &str,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> bool {
    let cursor = state.cursor();
    // 待定前缀：d/y/g 等第二键。
    if let Some(pending) = state.vim.pending {
        state.vim.pending = None;
        match (pending, key) {
            ('d', "d") => {
                delete_current_line(state, window, cx);
                return true;
            }
            ('y', "y") => {
                yank_current_line(state);
                cx.notify();
                return true;
            }
            ('g', "g") => {
                goto(state, 0, cx);
                return true;
            }
            _ => {} // 前缀作废，落下继续按普通键处理。
        }
    }
    match key {
        "h" => {
            let row = state.text().offset_to_point(cursor).row;
            let (start, _) = line_range(state, row);
            goto(state, prev_boundary(state.text(), cursor).max(start), cx);
        }
        "l" => {
            let row = state.text().offset_to_point(cursor).row;
            let (_, end) = line_range(state, row);
            // 行尾停留（`$` 位置，光标不进换行）。
            goto(state, next_boundary(state.text(), cursor).min(end), cx);
        }
        "j" => move_vertical(state, 1, cx),
        "k" => move_vertical(state, -1, cx),
        "w" => goto(state, word_forward(state, cursor), cx),
        "b" => goto(state, word_backward(state, cursor), cx),
        "0" => {
            let row = state.text().offset_to_point(cursor).row;
            goto(state, line_range(state, row).0, cx);
        }
        "$" => {
            let row = state.text().offset_to_point(cursor).row;
            let (start, end) = line_range(state, row);
            goto(state, end.max(start), cx);
        }
        "G" => {
            let last = line_count(state).saturating_sub(1);
            goto(state, line_range(state, last).0, cx);
        }
        "g" | "d" | "y" => {
            state.vim.pending = key.chars().next();
            cx.notify();
        }
        "x" => {
            let row = state.text().offset_to_point(cursor).row;
            let (_, end) = line_range(state, row);
            if cursor < end {
                delete_range(
                    state,
                    cursor..next_boundary(state.text(), cursor),
                    window,
                    cx,
                );
            }
        }
        "i" => to_insert(state, cx),
        "a" => {
            let row = state.text().offset_to_point(cursor).row;
            let (_, end) = line_range(state, row);
            if cursor < end {
                state.move_to(next_boundary(state.text(), cursor), None, cx);
            }
            to_insert(state, cx);
        }
        "o" => {
            let row = state.text().offset_to_point(cursor).row;
            let (_, end) = line_range(state, row);
            state.set_selected_range(end..end, cx);
            state.insert("\n", window, cx);
            to_insert(state, cx);
        }
        "v" => {
            state.vim.mode = VimMode::Visual;
            state.vim.anchor = Some(cursor);
            cx.notify();
        }
        "p" => put_register(state, window, cx),
        "u" => {
            state.undo(&Undo, window, cx);
        }
        "escape" => {
            // 清前缀 + 坍缩选区（`Input` 自身 Esc 被覆盖，此处补等价行为）。
            state.vim.pending = None;
            goto(state, cursor, cx);
        }
        "enter" => move_vertical(state, 1, cx),
        "backspace" => {
            let row = state.text().offset_to_point(cursor).row;
            let (start, _) = line_range(state, row);
            goto(state, prev_boundary(state.text(), cursor).max(start), cx);
        }
        "delete" => {
            let row = state.text().offset_to_point(cursor).row;
            let (_, end) = line_range(state, row);
            if cursor < end {
                delete_range(
                    state,
                    cursor..next_boundary(state.text(), cursor),
                    window,
                    cx,
                );
            }
        }
        // 数字前缀/命令/搜索/重复 v1 不做：吞掉不污染文本。
        _ => {}
    }
    true
}

/// 删除光标所在整行（末行含换行吃净；全文单行则清空内容）。
fn delete_current_line(state: &mut InputState, window: &mut Window, cx: &mut Context<InputState>) {
    let cursor = state.cursor();
    let text = state.text();
    let row = text.offset_to_point(cursor).row;
    let total = line_count(state);
    let (start, end) = line_range(state, row);
    if total <= 1 {
        delete_range(state, start..end.min(text.len()), window, cx);
    } else if row + 1 < total {
        let next_start = line_range(state, row + 1).0;
        delete_range(state, start..next_start.min(text.len()), window, cx);
    } else {
        // 末行：连前一个换行一起吃。
        delete_range(state, start.saturating_sub(1)..text.len(), window, cx);
    }
    let cursor = state.cursor().min(state.text().len());
    state.move_to(cursor, None, cx);
}

/// 复制光标所在整行进内部寄存器（不含换行，行粘贴）。
fn yank_current_line(state: &mut InputState) {
    let cursor = state.cursor();
    let text = state.text();
    let row = text.offset_to_point(cursor).row;
    let (start, end) = line_range(state, row);
    state.vim.register = text.slice(start..end.min(text.len())).to_string();
    state.vim.register_linewise = true;
}

/// 粘贴寄存器（行粘贴插到当前行下；字符粘贴插光标处；空寄存器无操作）。
fn put_register(state: &mut InputState, window: &mut Window, cx: &mut Context<InputState>) {
    if state.vim.register.is_empty() {
        return;
    }
    if state.vim.register_linewise {
        let cursor = state.cursor();
        let row = state.text().offset_to_point(cursor).row;
        let (_, end) = line_range(state, row);
        let text = state.vim.register.clone();
        if state.text().len() == 0 {
            state.set_selected_range(0..0, cx);
            state.insert(text.as_str(), window, cx);
        } else {
            state.set_selected_range(end..end, cx);
            let mut pasted = String::from("\n");
            pasted.push_str(&text);
            state.insert(pasted.as_str(), window, cx);
        }
    } else {
        let text = state.vim.register.clone();
        state.insert(text.as_str(), window, cx);
    }
}

/// visual 模式单键分发。
fn visual_key(
    state: &mut InputState,
    key: &str,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> bool {
    let cursor = state.cursor();
    match key {
        "h" | "j" | "k" | "l" | "w" | "b" | "0" | "$" | "G" => {
            let target = visual_motion_target(state, key, cursor);
            extend_to(state, target, cx);
        }
        "y" => {
            let range = state.selected_range();
            state.vim.register = state.text().slice(range).to_string();
            state.vim.register_linewise = false;
            to_normal(state, cx);
        }
        "d" | "x" => {
            let range = state.selected_range();
            if !range.is_empty() {
                delete_range(state, range, window, cx);
            }
            to_normal(state, cx);
        }
        "v" => to_normal(state, cx),
        "escape" => to_normal(state, cx),
        // 未列键直通忽略（`enter` 等走 `Input` 默认）。
        _ => {}
    }
    true
}

/// 可视位移目标（纯计算，不动状态；行列口径与 normal 一致）。
fn visual_motion_target(state: &InputState, key: &str, cursor: usize) -> usize {
    let text = state.text();
    let len = text.len();
    match key {
        "h" => {
            let row = text.offset_to_point(cursor).row;
            cursor.saturating_sub(1).max(line_range(state, row).0)
        }
        "l" => {
            let row = text.offset_to_point(cursor).row;
            (cursor + 1).min(line_range(state, row).1).min(len)
        }
        "j" => {
            let row = text.offset_to_point(cursor).row;
            let target = (row + 1).min(line_count(state).saturating_sub(1));
            char_offset_in_line(state, target, state.vim.preferred_col)
        }
        "k" => {
            let row = text.offset_to_point(cursor).row;
            let target = row.saturating_sub(1);
            char_offset_in_line(state, target, state.vim.preferred_col)
        }
        "w" => word_forward(state, cursor),
        "b" => word_backward(state, cursor),
        "0" => line_range(state, text.offset_to_point(cursor).row).0,
        "$" => {
            let row = text.offset_to_point(cursor).row;
            let (start, end) = line_range(state, row);
            end.max(start)
        }
        "G" => line_range(state, line_count(state).saturating_sub(1)).0,
        _ => cursor,
    }
}

/// Vim 按键总入口（`EditorState::vim_key` 经内部输入调用；启用且命中返回真）。
pub(crate) fn handle_key(
    state: &mut InputState,
    key: &str,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> bool {
    if !state.vim.enabled {
        return false;
    }
    match state.vim.mode {
        VimMode::Normal => normal_key(state, key, window, cx),
        VimMode::Visual => visual_key(state, key, window, cx),
        // Insert 下字母键无绑定到不了这里；`escape` 由 Editor 捕获直调 `escape_to_normal`。
        VimMode::Insert => false,
    }
}

/// Esc 行为（Editor `capture_key_down` 调用；`Input` 自身 Esc 照跑，不吞）。
pub(crate) fn escape_pressed(
    state: &mut InputState,
    _window: &mut Window,
    cx: &mut Context<InputState>,
) {
    if !state.vim.enabled {
        return;
    }
    match state.vim.mode {
        VimMode::Insert => {
            // 回 normal，光标左移一格（行首不动，vim 惯例）。
            let cursor = state.cursor();
            let row = state.text().offset_to_point(cursor).row;
            let (start, _) = line_range(state, row);
            state.vim.mode = VimMode::Normal;
            state.vim.pending = None;
            state.vim.anchor = None;
            state.move_to(prev_boundary(state.text(), cursor).max(start), None, cx);
        }
        VimMode::Visual => to_normal(state, cx),
        VimMode::Normal => {
            state.vim.pending = None;
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppContext as _;
    use crate::{Entity, Window};

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

    /// 建多行编辑器输入并开 vim（normal，首行行首；返回实体与 shadow 上下文）。
    fn vim_view<'a>(
        text: &str,
        cx: &'a mut crate::TestAppContext,
    ) -> (Entity<InputState>, &'a mut crate::VisualTestContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.set_value(text, window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.vim.enabled = true;
                state.vim.mode = VimMode::Normal;
                state.set_selected_range(0..0, cx);
            });
        });
        (state, cx)
    }

    /// 经实体派发 vim 键（`EditorState::vim_key` 同款路径）。
    fn press(state: &Entity<InputState>, key: &str, cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                assert!(handle_key(state, key, window, cx));
            });
        });
    }

    fn cursor_of(state: &Entity<InputState>, cx: &mut crate::VisualTestContext) -> usize {
        state.read_with(cx, |state, _| state.cursor())
    }

    fn text_of(state: &Entity<InputState>, cx: &mut crate::VisualTestContext) -> String {
        state.read_with(cx, |state, _| state.text().to_string())
    }

    /// hjkl 在行内移动（行首行尾钳制）。
    #[rgpui::test]
    fn hjkl_move_and_clamp(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            state.update(cx, |state, cx| state.set_value("abc\nde", window, cx));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|_, cx| {
            state.update(cx, |state, cx| {
                state.vim.enabled = true;
                state.vim.mode = VimMode::Normal;
                state.set_selected_range(0..0, cx);
            });
        });
        press(&state, "l", cx);
        press(&state, "l", cx);
        assert_eq!(cursor_of(&state, cx), 2);
        press(&state, "l", cx);
        // 行尾停留（不进换行）。
        assert_eq!(cursor_of(&state, cx), 3);
        press(&state, "h", cx);
        assert_eq!(cursor_of(&state, cx), 2);
        press(&state, "j", cx);
        // 下行同列。
        assert_eq!(cursor_of(&state, cx), 6);
        press(&state, "j", cx);
        // 末行停留。
        assert_eq!(cursor_of(&state, cx), 6);
        press(&state, "k", cx);
        assert_eq!(cursor_of(&state, cx), 2);
    }

    /// wb 在词间跳转。
    #[rgpui::test]
    fn word_motions(cx: &mut crate::TestAppContext) {
        let (state, cx) = vim_view("foo bar\nbaz", cx);
        press(&state, "w", cx);
        assert_eq!(cursor_of(&state, cx), 4);
        press(&state, "w", cx);
        assert_eq!(cursor_of(&state, cx), 8);
        press(&state, "b", cx);
        assert_eq!(cursor_of(&state, cx), 4);
        press(&state, "b", cx);
        assert_eq!(cursor_of(&state, cx), 0);
    }

    /// 0/$/gg/G 行文档跳转。
    #[rgpui::test]
    fn line_doc_jumps(cx: &mut crate::TestAppContext) {
        let (state, cx) = vim_view("abc\nde\nf", cx);
        press(&state, "$", cx);
        assert_eq!(cursor_of(&state, cx), 3);
        press(&state, "j", cx);
        press(&state, "$", cx);
        assert_eq!(cursor_of(&state, cx), 6);
        press(&state, "0", cx);
        assert_eq!(cursor_of(&state, cx), 4);
        press(&state, "G", cx);
        assert_eq!(cursor_of(&state, cx), 7);
        press(&state, "g", cx);
        press(&state, "g", cx);
        assert_eq!(cursor_of(&state, cx), 0);
    }

    /// x/dd/yy/p 编辑闭环。
    #[rgpui::test]
    fn edit_ops(cx: &mut crate::TestAppContext) {
        let (state, cx) = vim_view("abc\ndef\n", cx);
        // x 删光标字符。
        press(&state, "l", cx);
        press(&state, "x", cx);
        assert_eq!(text_of(&state, cx), "ac\ndef\n");
        // dd 删整行。
        press(&state, "d", cx);
        press(&state, "d", cx);
        assert_eq!(text_of(&state, cx), "def\n");
        // yy + p 行粘贴。
        press(&state, "y", cx);
        press(&state, "y", cx);
        press(&state, "p", cx);
        assert_eq!(text_of(&state, cx), "def\ndef\n");
    }

    /// u 撤销上一次修改（独立用例：与前序操作隔 burst，单组回退）。
    #[rgpui::test]
    fn undo_after_vim_edit(cx: &mut crate::TestAppContext) {
        let (state, cx) = vim_view("abc", cx);
        press(&state, "x", cx);
        assert_eq!(text_of(&state, cx), "bc");
        press(&state, "u", cx);
        assert_eq!(text_of(&state, cx), "abc");
    }

    /// i/a/o/v 模式切换。
    #[rgpui::test]
    fn mode_switches(cx: &mut crate::TestAppContext) {
        let (state, cx) = vim_view("abc", cx);
        let mode = |state: &Entity<InputState>, cx: &mut crate::VisualTestContext| {
            state.read_with(cx, |state, _| state.vim.mode)
        };
        press(&state, "i", cx);
        assert_eq!(mode(&state, cx), VimMode::Insert);
        cx.update(|window, cx| {
            state.update(cx, |state, cx| escape_pressed(state, window, cx));
        });
        assert_eq!(mode(&state, cx), VimMode::Normal);
        press(&state, "v", cx);
        assert_eq!(mode(&state, cx), VimMode::Visual);
        press(&state, "l", cx);
        // 可视扩展选区 0..1。
        assert_eq!(state.read_with(cx, |state, _| state.selected_range()), 0..1);
        press(&state, "y", cx);
        assert_eq!(mode(&state, cx), VimMode::Normal);
        // yank 后 p 粘贴 `a`。
        press(&state, "0", cx);
        press(&state, "p", cx);
        assert_eq!(text_of(&state, cx), "aabc");
    }

    /// 未启用时按键直通（不吞）。
    #[rgpui::test]
    fn disabled_passes_through(cx: &mut crate::TestAppContext) {
        let (probe, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).multi_line(true));
            Probe { state }
        });
        let state = probe.read_with(cx, |probe, _| probe.state.clone());
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                assert!(!handle_key(state, "h", window, cx));
            });
        });
    }

    /// vim 键位已注册（normal 下 `h`/`G`/`enter`/`escape` 命中 `VimKey`）。
    #[rgpui::test]
    fn bindings_registered_for_vim_contexts(cx: &mut crate::TestAppContext) {
        use crate::{KeyContext, Keystroke};
        cx.update(crate::input_ui::init);
        let strokes = |keys: &[&str]| {
            keys.iter()
                .map(|key| Keystroke::parse(key).unwrap())
                .collect::<Vec<_>>()
        };
        let hits: Vec<bool> = cx.read(|cx| {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let stack = vec![KeyContext::parse("VimNormal").unwrap()];
            ["h", "G", "enter", "escape", "$"]
                .into_iter()
                .map(|key| {
                    let (bindings, _) = keymap.bindings_for_input(&strokes(&[key]), &stack);
                    bindings.iter().any(|binding| {
                        binding
                            .action
                            .as_any()
                            .downcast_ref::<VimKey>()
                            .is_some_and(|action| action.key.as_ref() == key)
                    })
                })
                .collect()
        });
        assert_eq!(hits, vec![true; 5]);
        // insert 下字母键无绑定（键入直通）。
        let insert_hits: Vec<bool> = cx.read(|cx| {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let stack = vec![KeyContext::parse("VimInsert").unwrap()];
            ["h", "x"]
                .into_iter()
                .map(|key| {
                    let (bindings, _) = keymap.bindings_for_input(&strokes(&[key]), &stack);
                    bindings
                        .iter()
                        .any(|binding| binding.action.as_any().downcast_ref::<VimKey>().is_some())
                })
                .collect()
        });
        assert_eq!(insert_hits, vec![false, false]);
    }
}
