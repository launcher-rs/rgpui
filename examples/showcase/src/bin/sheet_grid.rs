//! 电子表格网格演示：`SheetGrid` + `FormulaBar` 绑定一个 8×6 的迷你表格。
//!
//! 鼠标拖拽选区、Shift 扩展、行列头点击整行/整列、方向键/PageUp/PageDown 移动、
//! Ctrl+A 全选、双击或 Enter/F2 请求编辑、Delete 清空、右下角填充柄向下/向右填充。
//! 选区变化会同步到公式栏的名称框与公式框，公式栏回车写回活动单元格。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::formula_bar::{FormulaBar, FormulaBarEvent, FormulaBarState};
use rgpui::sheet_grid::{
    CellRef, SheetDelegate, SheetGrid, SheetGridEvent, SheetGridState, column_label,
};
use rgpui::{
    App, Bounds, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px,
    rgb, size, v_flex,
};
use rgpui_platform::application;

/// 列数。
const COLS: usize = 6;
/// 行数。
const ROWS: usize = 8;

/// 单元格引用文本，如 `B3`。
fn cell_ref(cell: CellRef) -> String {
    format!("{}{}", column_label(cell.col), cell.row + 1)
}

/// 解析 `B3` 形式的引用为单元格坐标。
fn parse_ref(text: &str) -> Option<CellRef> {
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
    if row == 0 {
        return None;
    }
    Some(CellRef::new(row - 1, col))
}

/// 演示表格数据委托。
struct DemoSheet {
    /// 单元格文本（`[row][col]`）。
    cells: Vec<Vec<String>>,
}

impl DemoSheet {
    /// 创建预填数据的表格。
    fn new() -> Self {
        let mut cells = vec![vec![String::new(); COLS]; ROWS];
        cells[0][0] = "季度".to_string();
        cells[0][1] = "收入".to_string();
        cells[0][2] = "成本".to_string();
        cells[0][3] = "利润".to_string();
        for (row, line) in cells.iter_mut().enumerate().skip(1) {
            line[0] = format!("Q{row}");
            line[1] = (1200 * row).to_string();
            line[2] = (700 * row).to_string();
            line[3] = format!("=B{}-C{}", row + 1, row + 1);
        }
        Self { cells }
    }

    /// 读取单元格文本。
    fn value(&self, cell: CellRef) -> String {
        self.cells
            .get(cell.row)
            .and_then(|row| row.get(cell.col))
            .cloned()
            .unwrap_or_default()
    }

    /// 写入单元格文本。
    fn set(&mut self, cell: CellRef, value: String) {
        if let Some(row) = self.cells.get_mut(cell.row)
            && let Some(slot) = row.get_mut(cell.col)
        {
            *slot = value;
        }
    }
}

impl SheetDelegate for DemoSheet {
    fn rows_count(&self, _cx: &App) -> usize {
        self.cells.len()
    }

    fn cols_count(&self, _cx: &App) -> usize {
        self.cells.first().map_or(0, Vec::len)
    }

    fn text(&self, cell: CellRef, _cx: &App) -> SharedString {
        self.value(cell).into()
    }
}

/// 演示视图。
struct SheetGridDemo {
    /// 网格状态。
    grid: Entity<SheetGridState<DemoSheet>>,
    /// 公式栏状态。
    bar: Entity<FormulaBarState>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 状态栏文本。
    status: String,
}

impl SheetGridDemo {
    /// 创建演示视图。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let bar = cx.new(|cx| FormulaBarState::new(window, cx));
        cx.observe(&bar, |_, _, cx| cx.notify()).detach();

        let grid = cx.new(|cx| SheetGridState::new(DemoSheet::new(), cx));
        cx.observe(&grid, |_, _, cx| cx.notify()).detach();

        // 网格事件 → 同步公式栏与数据。
        cx.subscribe_in(
            &grid,
            window,
            |this, _, event: &SheetGridEvent, window, cx| match event {
                SheetGridEvent::SelectionChanged(range) => {
                    let cell = range.cursor;
                    let value = this.grid.read(cx).delegate().value(cell);
                    let bar = this.bar.clone();
                    bar.update(cx, |bar, cx| {
                        bar.set_name(cell_ref(cell), window, cx);
                        bar.set_formula(value, window, cx);
                    });
                    this.status = format!(
                        "选区 {}:{}（{} 格）",
                        cell_ref(range.min()),
                        cell_ref(range.max()),
                        range.len()
                    );
                    cx.notify();
                }
                SheetGridEvent::EditRequested(cell) => {
                    this.grid
                        .update(cx, |grid, cx| grid.set_editing(Some(*cell), cx));
                    this.status = format!("编辑 {}（在公式栏输入后回车）", cell_ref(*cell));
                    cx.notify();
                }
                SheetGridEvent::Clear(range) => {
                    let value = String::new();
                    let cells: Vec<CellRef> = range
                        .rows()
                        .flat_map(|row| range.cols().map(move |col| CellRef::new(row, col)))
                        .collect();
                    this.grid.update(cx, |grid, cx| {
                        for cell in cells {
                            grid.delegate_mut().set(cell, value.clone());
                        }
                        cx.notify();
                    });
                    this.status = "已清空选区".to_string();
                    cx.notify();
                }
                SheetGridEvent::Fill { source, target } => {
                    let value = this.grid.read(cx).delegate().value(source.min());
                    let cells: Vec<CellRef> = target
                        .rows()
                        .flat_map(|row| target.cols().map(move |col| CellRef::new(row, col)))
                        .collect();
                    this.grid.update(cx, |grid, cx| {
                        for cell in cells {
                            grid.delegate_mut().set(cell, value.clone());
                        }
                        cx.notify();
                    });
                    this.status = format!(
                        "填充 {} → {}",
                        cell_ref(source.min()),
                        cell_ref(target.max())
                    );
                    cx.notify();
                }
                SheetGridEvent::HeaderClicked { col, row } => {
                    this.status = match (row, col) {
                        (Some(row), _) => format!("选中第 {} 行", row + 1),
                        (_, Some(col)) => format!("选中 {} 列", column_label(*col)),
                        _ => "选中表头".to_string(),
                    };
                    cx.notify();
                }
            },
        )
        .detach();

        // 公式栏事件 → 写回网格。
        cx.subscribe_in(
            &bar,
            window,
            |this, _, event: &FormulaBarEvent, window, cx| match event {
                FormulaBarEvent::NameSubmit(name) => {
                    if let Some(cell) = parse_ref(name) {
                        this.grid
                            .update(cx, |grid, cx| grid.set_active_cell(cell, cx));
                        this.status = format!("跳转到 {}", cell_ref(cell));
                    } else {
                        this.status = format!("无法识别的引用：{name}");
                    }
                    cx.notify();
                }
                FormulaBarEvent::FormulaSubmit(text) => {
                    let active = this.grid.read(cx).active_cell();
                    let text = text.to_string();
                    this.grid.update(cx, |grid, cx| {
                        grid.delegate_mut().set(active, text.clone());
                        grid.set_editing(None, cx);
                        cx.notify();
                    });
                    this.status = format!("{} ← {}", cell_ref(active), text);
                    cx.notify();
                }
                FormulaBarEvent::Cancel => {
                    let active = this.grid.read(cx).active_cell();
                    let value = this.grid.read(cx).delegate().value(active);
                    let bar = this.bar.clone();
                    bar.update(cx, |bar, cx| bar.set_formula(value, window, cx));
                    this.grid.update(cx, |grid, cx| grid.set_editing(None, cx));
                    this.status = "已取消编辑".to_string();
                    cx.notify();
                }
                FormulaBarEvent::InsertFunction => {
                    let current = this.bar.read(cx).formula(cx);
                    let next = if current.trim().is_empty() || current.trim() == "=" {
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
            grid,
            bar,
            focus_handle,
            status: "拖拽选区 · 双击编辑 · Ctrl+A 全选 · 拖填充柄复制".to_string(),
        }
    }
}

impl Render for SheetGridDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("sheet-grid-demo")
            .key_context("SheetGridDemo")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(rgb(0xffffff))
            .child(FormulaBar::new(&self.bar))
            .child(
                div()
                    .flex_grow_1()
                    .min_h(px(0.0))
                    .border_t_1()
                    .border_color(rgb(0xe2e8f0))
                    .child(SheetGrid::new(&self.grid)),
            )
            .child(
                h_flex()
                    .px(px(12.0))
                    .py(px(6.0))
                    .border_t_1()
                    .border_color(rgb(0xe2e8f0))
                    .text_sm()
                    .text_color(rgb(0x64748b))
                    .child(self.status.clone()),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| SheetGridDemo::new(window, cx)),
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
