//! 公式栏演示：名称框 + fx + 公式输入，绑定一个 4×5 的迷你「单元格」模型。
//! 名称框输入 `B3` 跳转、公式框回车写入当前单元格、fx 插入 `=SUM(`、Esc 取消。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::formula_bar::{FormulaBar, FormulaBarEvent, FormulaBarState};
use rgpui::{
    App, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled, Window,
    WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 列数。
const COLS: usize = 4;
/// 行数。
const ROWS: usize = 5;

/// 演示视图。
struct FormulaBarDemo {
    /// 公式栏状态。
    bar: Entity<FormulaBarState>,
    /// 迷你单元格内容（`[row][col]`）。
    cells: Vec<Vec<String>>,
    /// 当前活动单元格（row, col）。
    active: (usize, usize),
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 状态栏文本。
    status: String,
}

/// 将列号（0 起）转为列名（A、B、…）。
fn column_name(col: usize) -> String {
    let mut col = col;
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    name
}

/// 单元格引用，如 `B3`。
fn cell_ref(row: usize, col: usize) -> String {
    format!("{}{}", column_name(col), row + 1)
}

/// 解析 `B3` 形式的引用为 (row, col)。
fn parse_ref(text: &str) -> Option<(usize, usize)> {
    let text = text.trim().to_ascii_uppercase();
    let col_end = text.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = text.split_at(col_end);
    if letters.is_empty() || letters.len() > 2 {
        return None;
    }
    let mut col = 0usize;
    for ch in letters.chars() {
        if !ch.is_ascii_uppercase() {
            return None;
        }
        col = col * 26 + (ch as usize - 'A' as usize + 1);
    }
    let col = col.checked_sub(1)?;
    let row: usize = digits.parse().ok()?;
    if row == 0 || col >= COLS {
        return None;
    }
    Some((row - 1, col))
}

impl FormulaBarDemo {
    /// 创建演示视图。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let bar = cx.new(|cx| FormulaBarState::new(window, cx));
        // 展开态等变化时重绘。
        cx.observe(&bar, |_, _, cx| cx.notify()).detach();
        // 接收公式栏事件。
        cx.subscribe_in(
            &bar,
            window,
            |this, _, event: &FormulaBarEvent, window, cx| match event {
                FormulaBarEvent::NameSubmit(name) => {
                    if let Some((row, col)) = parse_ref(name) {
                        this.select(row, col, window, cx);
                        this.status = format!("跳转到 {}", cell_ref(row, col));
                    } else {
                        this.status = format!("无法识别的引用：{name}");
                    }
                    cx.notify();
                }
                FormulaBarEvent::FormulaSubmit(text) => {
                    let (row, col) = this.active;
                    this.cells[row][col] = text.to_string();
                    this.status = format!("{} ← {text}", cell_ref(row, col));
                    cx.notify();
                }
                FormulaBarEvent::Cancel => {
                    let (row, col) = this.active;
                    let value = this.cells[row][col].clone();
                    let bar = this.bar.clone();
                    bar.update(cx, |bar, cx| bar.set_formula(value, window, cx));
                    this.status = "已取消编辑".to_string();
                    cx.notify();
                }
                FormulaBarEvent::InsertFunction => {
                    let current = this.bar.read(cx).formula(cx);
                    let next = if current.trim().is_empty() {
                        "=SUM(".to_string()
                    } else if current.starts_with('=') {
                        current.to_string()
                    } else {
                        format!("={current}")
                    };
                    let bar = this.bar.clone();
                    bar.update(cx, |bar, cx| bar.set_formula(next, window, cx));
                    this.status = "插入函数".to_string();
                    cx.notify();
                }
            },
        )
        .detach();

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            bar,
            cells: vec![vec![String::new(); COLS]; ROWS],
            active: (0, 0),
            focus_handle,
            status: "选择单元格或使用公式栏编辑".to_string(),
        }
    }

    /// 选中单元格并同步公式栏显示。
    fn select(&mut self, row: usize, col: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active = (row, col);
        let value = self.cells[row][col].clone();
        let bar = self.bar.clone();
        bar.update(cx, |bar, cx| {
            bar.set_name(cell_ref(row, col), window, cx);
            bar.set_formula(value, window, cx);
        });
    }
}

impl Render for FormulaBarDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .gap(px(1.0))
            .child(div().w(px(36.0)))
            .children((0..COLS).map(|col| {
                div()
                    .w(px(90.0))
                    .px(px(6.0))
                    .py(px(2.0))
                    .text_sm()
                    .font_weight(rgpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0x475569))
                    .child(column_name(col))
            }));

        let rows = (0..ROWS).map(|row| {
            let cells = (0..COLS).map(|col| {
                let is_active = self.active == (row, col);
                div()
                    .id(("cell", row * COLS + col))
                    .w(px(90.0))
                    .h(px(26.0))
                    .px(px(6.0))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(rgb(0xe2e8f0))
                    .text_sm()
                    .when(is_active, |this| {
                        this.border_color(rgb(0x2563eb)).bg(rgb(0xeff6ff))
                    })
                    .cursor_pointer()
                    .child(self.cells[row][col].clone())
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.select(row, col, window, cx);
                        this.status = format!("选中 {}", cell_ref(row, col));
                        cx.notify();
                    }))
            });

            h_flex()
                .gap(px(1.0))
                .child(
                    div()
                        .w(px(36.0))
                        .h(px(26.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(rgb(0x475569))
                        .child((row + 1).to_string()),
                )
                .children(cells)
        });

        v_flex()
            .id("formula-bar-demo")
            .key_context("FormulaBarDemo")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(rgb(0xffffff))
            .child(FormulaBar::new(&self.bar))
            .child(
                v_flex()
                    .gap(px(1.0))
                    .p(px(12.0))
                    .child(header)
                    .children(rows),
            )
            .child(
                div()
                    .px(px(12.0))
                    .py(px(6.0))
                    .text_sm()
                    .text_color(rgb(0x64748b))
                    .child(format!(
                        "活动单元格：{}    状态：{}",
                        cell_ref(self.active.0, self.active.1),
                        self.status
                    )),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| FormulaBarDemo::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run_example();
}
