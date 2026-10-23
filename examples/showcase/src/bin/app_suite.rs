//! 应用套件基座整合演示：一份命令表驱动 Ribbon / 按键提示 / 快速访问工具栏 /
//! 应用菜单（Backstage）/ 分页视口（PageView + Viewport + 参考线）。
//!
//! 与单功能示例（`ribbon` / `qat` / `backstage` / `page_view`）不同，本例把套件基座的
//! 各块拼成一个可用的编辑器骨架：每个入口只写命令 id，点击经 `App::dispatch_command`
//! 派发到同一批 `on_action`，改动同一份文档模型；状态栏回显
//! `CommandRegistry::label_for` 取出的标签，用来证明「UI 文案」与「命令清单」同源。
//!
//! 运行：`cargo run -p showcase --bin app_suite`（依赖 `ribbon` + `document` feature，
//! showcase 默认开启）。按 Alt/F10 显示按键提示，再按提示键即可纯键盘操作功能区。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::backstage::{Backstage, BackstageState, BackstageTab};
use rgpui::page_view::{PageArrangement, PageLabels, PageLayout, PageMargins, PageSetup, PageView};
use rgpui::qat::{QatItem, QuickAccessToolbar};
use rgpui::ribbon::{
    Ribbon, RibbonContext, RibbonDropdownItem, RibbonGalleryItem, RibbonGroup, RibbonItem,
    RibbonState, RibbonTab,
};
use rgpui::viewport::Viewport;
use rgpui::{
    App, Bounds, ClickEvent, CommandDescriptor, CommandRegistry, Context, Entity, FocusHandle,
    IconName, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Render, ScrollWheelEvent, SharedString, Size, Window, WindowBounds, WindowOptions, actions,
    div, h_flex, point, prelude::*, px, register_command, relative, rgb, size, v_flex,
};
use rgpui_platform::application;

actions!(
    app_suite,
    [
        New,
        Open,
        Save,
        Print,
        Paste,
        PasteSpecial,
        Cut,
        Copy,
        Bold,
        StyleNormal,
        StyleTitle,
        StyleEmphasis,
        InsertTable,
        InsertPicture,
        NewPage,
        ZoomIn,
        ZoomOut,
        ZoomActual,
        ZoomFit,
        LayoutSingle,
        LayoutTwoUp,
        LayoutCover,
        ToggleMargins,
        ToggleGuides,
        CollapseRibbon,
        Recalc
    ]
);

/// 命令 id 前缀（与 `actions!` 生成的动作名一致，派发时按此名构建动作）。
const NS: &str = "app_suite::";

/// 构造一条命令描述符：分类、按键提示与默认快捷键集中在此声明。
fn command(
    name: &str,
    label: &str,
    category: &str,
    keytip: Option<&str>,
    keystroke: Option<&str>,
) -> CommandDescriptor {
    let mut descriptor =
        CommandDescriptor::new(format!("{NS}{name}"), label).with_category(category);
    if let Some(keytip) = keytip {
        descriptor = descriptor.with_keytip(keytip);
    }
    if let Some(keystroke) = keystroke {
        descriptor = descriptor.with_keystroke(keystroke);
    }
    descriptor
}

/// 「新建」命令。
fn cmd_new() -> CommandDescriptor {
    command("New", "新建", "文件", Some("N"), Some("ctrl-n"))
}
/// 「打开」命令。
fn cmd_open() -> CommandDescriptor {
    command("Open", "打开", "文件", Some("O"), Some("ctrl-o"))
}
/// 「保存」命令。
fn cmd_save() -> CommandDescriptor {
    command("Save", "保存", "文件", Some("S"), Some("ctrl-s"))
}
/// 「打印」命令。
fn cmd_print() -> CommandDescriptor {
    command("Print", "打印", "文件", Some("U"), Some("ctrl-p"))
}
/// 「粘贴」命令。
fn cmd_paste() -> CommandDescriptor {
    command("Paste", "粘贴", "剪贴板", Some("V"), Some("ctrl-v"))
}
/// 「选择性粘贴」命令（粘贴分裂按钮的下拉项）。
fn cmd_paste_special() -> CommandDescriptor {
    command("PasteSpecial", "选择性粘贴", "剪贴板", None, None)
}
/// 「剪切」命令。
fn cmd_cut() -> CommandDescriptor {
    command("Cut", "剪切", "剪贴板", Some("X"), Some("ctrl-x"))
}
/// 「复制」命令。
fn cmd_copy() -> CommandDescriptor {
    command("Copy", "复制", "剪贴板", Some("C"), Some("ctrl-c"))
}
/// 「加粗」命令（切换）。
fn cmd_bold() -> CommandDescriptor {
    command("Bold", "加粗", "字体", Some("B"), Some("ctrl-b"))
}
/// 「常规」样式命令（样式库库项）。
fn cmd_style_normal() -> CommandDescriptor {
    command("StyleNormal", "常规", "样式", None, None)
}
/// 「标题」样式命令（样式库库项）。
fn cmd_style_title() -> CommandDescriptor {
    command("StyleTitle", "标题", "样式", None, None)
}
/// 「强调」样式命令（样式库库项）。
fn cmd_style_emphasis() -> CommandDescriptor {
    command("StyleEmphasis", "强调", "样式", None, None)
}
/// 「插入表格」命令。
fn cmd_insert_table() -> CommandDescriptor {
    command("InsertTable", "表格", "插入", Some("T"), None)
}
/// 「插入图片」命令。
fn cmd_insert_picture() -> CommandDescriptor {
    command("InsertPicture", "图片", "插入", Some("I"), None)
}
/// 「新建页」命令。
fn cmd_new_page() -> CommandDescriptor {
    command("NewPage", "分页", "插入", Some("G"), None)
}
/// 「放大」命令。
fn cmd_zoom_in() -> CommandDescriptor {
    command("ZoomIn", "放大", "视图", Some("Z"), Some("ctrl-up"))
}
/// 「缩小」命令。
fn cmd_zoom_out() -> CommandDescriptor {
    command("ZoomOut", "缩小", "视图", Some("Q"), Some("ctrl-down"))
}
/// 「实际大小」命令。
fn cmd_zoom_actual() -> CommandDescriptor {
    command("ZoomActual", "100%", "视图", Some("A"), Some("ctrl-0"))
}
/// 「适应窗口」命令。
fn cmd_zoom_fit() -> CommandDescriptor {
    command(
        "ZoomFit",
        "适应窗口",
        "视图",
        Some("F"),
        Some("ctrl-shift-f"),
    )
}
/// 「单页排布」命令（排布下拉项，不单独占按键提示）。
fn cmd_layout_single() -> CommandDescriptor {
    command("LayoutSingle", "单页", "视图", None, None)
}
/// 「对开排布」命令（排布下拉项）。
fn cmd_layout_two_up() -> CommandDescriptor {
    command("LayoutTwoUp", "对开", "视图", None, None)
}
/// 「封面对开排布」命令（排布下拉项）。
fn cmd_layout_cover() -> CommandDescriptor {
    command("LayoutCover", "封面对开", "视图", None, None)
}
/// 「窄边距」命令（在常规与窄边距之间切换）。
fn cmd_toggle_margins() -> CommandDescriptor {
    command("ToggleMargins", "窄边距", "视图", Some("M"), None)
}
/// 「内容参考线」命令（切换）。
fn cmd_toggle_guides() -> CommandDescriptor {
    command("ToggleGuides", "内容参考线", "视图", Some("D"), None)
}
/// 「折叠功能区」命令（切换）。
fn cmd_collapse_ribbon() -> CommandDescriptor {
    command("CollapseRibbon", "折叠功能区", "视图", Some("R"), None)
}
/// 「重新计算」命令（电子表格基座同款语义：补齐待重算的合计）。
fn cmd_recalc() -> CommandDescriptor {
    command("Recalc", "重新计算", "数据", Some("L"), Some("f9"))
}

register_command!(cmd_new);
register_command!(cmd_open);
register_command!(cmd_save);
register_command!(cmd_print);
register_command!(cmd_paste);
register_command!(cmd_paste_special);
register_command!(cmd_cut);
register_command!(cmd_copy);
register_command!(cmd_bold);
register_command!(cmd_style_normal);
register_command!(cmd_style_title);
register_command!(cmd_style_emphasis);
register_command!(cmd_insert_table);
register_command!(cmd_insert_picture);
register_command!(cmd_new_page);
register_command!(cmd_zoom_in);
register_command!(cmd_zoom_out);
register_command!(cmd_zoom_actual);
register_command!(cmd_zoom_fit);
register_command!(cmd_layout_single);
register_command!(cmd_layout_two_up);
register_command!(cmd_layout_cover);
register_command!(cmd_toggle_margins);
register_command!(cmd_toggle_guides);
register_command!(cmd_collapse_ribbon);
register_command!(cmd_recalc);

/// 段落样式（功能区「样式库」的三个库项对应三档）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocStyle {
    /// 常规正文。
    Normal,
    /// 标题（更大更深）。
    Title,
    /// 强调（琥珀色）。
    Emphasis,
}

/// 文档块：段落 / 表格 / 图片占位。
enum Block {
    /// 一段正文。
    Paragraph(SharedString),
    /// 表格：行名与数值，`total` 为 `None` 表示尚未重算。
    Table {
        rows: Vec<(SharedString, f64)>,
        total: Option<f64>,
    },
    /// 图片占位（不做真实解码，只演示块级内容）。
    Picture(SharedString),
}

/// 一页文档（分页由应用层维护，框架只出几何与纸页外观）。
struct Page {
    /// 本页内容块。
    blocks: Vec<Block>,
}

/// 造一张示例表格（合计留空，等「重新计算」补齐）。
fn sample_table() -> Block {
    const VALUES: [(&str, f64); 4] = [
        ("一月", 120.0),
        ("二月", 180.0),
        ("三月", 90.0),
        ("四月", 240.0),
    ];
    Block::Table {
        rows: VALUES
            .iter()
            .map(|(name, value)| (SharedString::from(*name), *value))
            .collect(),
        total: None,
    }
}

/// 初始文档：两页，含说明段落与一张待重算的表格。
fn starter_pages() -> Vec<Page> {
    vec![
        Page {
            blocks: vec![
                Block::Paragraph("应用套件基座 · 季度报表".into()),
                Block::Paragraph(
                    "顶部三处入口（快速访问工具栏、功能区、文件菜单）都只写命令 id，\
                     落到同一批动作处理上；改一处命令清单，三处 UI 同步生效。"
                        .into(),
                ),
                sample_table(),
            ],
        },
        Page {
            blocks: vec![Block::Paragraph(
                "第二页：插入选项卡的「表格 / 图片 / 分页」改动的就是这份模型；\
                 视图选项卡的缩放与排布只改视口和分页设置，不动内容。"
                    .into(),
            )],
        },
    ]
}

/// 页眉页脚文本。
struct SuiteLabels {
    /// 文档标题。
    title: SharedString,
    /// 总页数。
    pages: usize,
}

impl PageLabels for SuiteLabels {
    fn header(&self, _page: usize, _cx: &App) -> SharedString {
        self.title.clone()
    }

    fn footer(&self, page: usize, _cx: &App) -> SharedString {
        format!("第 {} / {} 页", page + 1, self.pages).into()
    }
}

/// 演示视图：套件基座各块共用一份命令表。
struct AppSuite {
    /// 文档页。
    pages: Vec<Page>,
    /// 当前段落样式。
    style: DocStyle,
    /// 是否加粗。
    bold: bool,
    /// 是否有未保存改动。
    dirty: bool,
    /// 分页设置（纸张 / 边距 / 页眉页脚 / 页间距）。
    setup: PageSetup,
    /// 页面排布方式。
    arrangement: PageArrangement,
    /// 是否显示内容区参考线。
    guides: bool,
    /// 视口（缩放 + 平移）。
    viewport: Viewport,
    /// 功能区状态。
    ribbon: Entity<RibbonState>,
    /// 应用菜单状态。
    backstage: Entity<BackstageState>,
    /// 焦点句柄。
    focus_handle: FocusHandle,
    /// 最近执行的命令回显。
    last: SharedString,
    /// 画布可用尺寸（供适应窗口使用）。
    view_size: Size<Pixels>,
    /// 缩放锚点（最近鼠标位置）。
    anchor: Point<Pixels>,
    /// 是否正在拖拽平移。
    dragging: bool,
    /// 上次指针位置。
    last_pointer: Point<Pixels>,
}

impl AppSuite {
    /// 创建演示视图：建子状态、聚焦自身。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ribbon = cx.new(RibbonState::new);
        let backstage = cx.new(BackstageState::new);
        // 子状态变化（切选项卡、开合应用菜单）都要重绘本视图。
        cx.observe(&ribbon, |_, _, cx| cx.notify()).detach();
        cx.observe(&backstage, |_, _, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            pages: starter_pages(),
            style: DocStyle::Normal,
            bold: false,
            dirty: false,
            setup: PageSetup {
                margins: PageMargins::NORMAL,
                ..PageSetup::default()
            },
            arrangement: PageArrangement::Single,
            guides: true,
            viewport: Viewport::new()
                .with_zoom(0.5)
                .with_offset(point(px(64.0), px(16.0))),
            ribbon,
            backstage,
            focus_handle,
            last: "（尚未执行命令）".into(),
            view_size: size(px(960.0), px(600.0)),
            anchor: point(px(480.0), px(300.0)),
            dragging: false,
            last_pointer: point(px(0.0), px(0.0)),
        }
    }

    /// 记录一次命令执行：标签取自注册表，与 UI 文案同源。
    fn record(&mut self, name: &str, cx: &mut Context<Self>) {
        let id = format!("{NS}{name}");
        let label = CommandRegistry::global().label_for(&id);
        self.last = format!("{label}（{id}）").into();
        cx.notify();
    }

    /// 标记文档已改动。
    fn touch(&mut self) {
        self.dirty = true;
    }

    /// 往当前页追加一个块（演示口径：始终编辑最后一页）。
    fn push_block(&mut self, block: Block) {
        let page = self.pages.len().saturating_sub(1);
        self.pages[page].blocks.push(block);
        self.touch();
    }

    /// 打开 / 关闭应用菜单，并把焦点交给面板（Esc 才生效）。
    fn toggle_backstage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.backstage.update(cx, |state, cx| state.toggle(cx));
        let focus = self.backstage.read(cx).focus_handle();
        window.focus(&focus, cx);
    }

    /// 待重算的表格数量（状态栏提示用）。
    fn stale_tables(&self) -> usize {
        self.pages
            .iter()
            .flat_map(|page| page.blocks.iter())
            .filter(|block| matches!(block, Block::Table { total: None, .. }))
            .count()
    }

    /// 依当前设置与页数构造分页布局。
    fn layout(&self) -> PageLayout {
        PageLayout::new(self.setup, self.pages.len()).with_arrangement(self.arrangement)
    }

    /// 排布方式的中文标签。
    fn arrangement_label(&self) -> &'static str {
        match self.arrangement {
            PageArrangement::Single => "单页",
            PageArrangement::TwoUp => "对开",
            PageArrangement::TwoUpCover => "封面对开",
        }
    }

    /// 功能区选项卡：所有条目只写命令 id，标签由命令描述符提供。
    ///
    /// 按键提示在整块功能区里必须互不相同（前缀解析按字符匹配候选项），
    /// 选项卡用 H/J/N/K，条目用其余字母。
    fn ribbon_tabs(&self, collapsed: bool) -> Vec<RibbonTab> {
        vec![
            RibbonTab::new("home", "开始")
                .keytip("H")
                .group(
                    RibbonGroup::new("剪贴板")
                        .item(
                            RibbonItem::split("app_suite::Paste")
                                .label("粘贴")
                                .keytip("V")
                                .icon(IconName::Copy)
                                .large()
                                .item(
                                    RibbonDropdownItem::new("app_suite::PasteSpecial")
                                        .label("选择性粘贴"),
                                ),
                        )
                        .item(
                            RibbonItem::button("app_suite::Cut")
                                .label("剪切")
                                .icon(IconName::Delete),
                        )
                        .item(
                            RibbonItem::button("app_suite::Copy")
                                .label("复制")
                                .icon(IconName::Copy),
                        ),
                )
                .group(
                    RibbonGroup::new("字体").item(
                        RibbonItem::toggle("app_suite::Bold")
                            .label("加粗")
                            .keytip("B")
                            .icon(IconName::ALargeSmall)
                            .checked(self.bold),
                    ),
                )
                .group(
                    RibbonGroup::new("样式").item(
                        RibbonItem::gallery("样式库")
                            .keytip("Y")
                            .columns(1)
                            .large()
                            .icon(IconName::Palette)
                            .item(
                                RibbonGalleryItem::new("app_suite::StyleNormal")
                                    .label("常规")
                                    .icon(IconName::File)
                                    .selected(self.style == DocStyle::Normal),
                            )
                            .item(
                                RibbonGalleryItem::new("app_suite::StyleTitle")
                                    .label("标题")
                                    .icon(IconName::Star)
                                    .selected(self.style == DocStyle::Title),
                            )
                            .item(
                                RibbonGalleryItem::new("app_suite::StyleEmphasis")
                                    .label("强调")
                                    .icon(IconName::Palette)
                                    .selected(self.style == DocStyle::Emphasis),
                            ),
                    ),
                ),
            RibbonTab::new("insert", "插入").keytip("J").group(
                RibbonGroup::new("插图")
                    .item(
                        RibbonItem::button("app_suite::InsertTable")
                            .label("表格")
                            .keytip("T")
                            .icon(IconName::LayoutDashboard)
                            .large(),
                    )
                    .item(
                        RibbonItem::button("app_suite::InsertPicture")
                            .label("图片")
                            .keytip("I")
                            .icon(IconName::GalleryVerticalEnd),
                    )
                    .item(
                        RibbonItem::button("app_suite::NewPage")
                            .label("分页")
                            .keytip("G")
                            .icon(IconName::BookOpen),
                    ),
            ),
            RibbonTab::new("view", "视图")
                .keytip("N")
                .group(
                    RibbonGroup::new("显示")
                        .item(
                            RibbonItem::button("app_suite::ZoomIn")
                                .label("放大")
                                .keytip("Z")
                                .icon(IconName::Plus),
                        )
                        .item(
                            RibbonItem::button("app_suite::ZoomOut")
                                .label("缩小")
                                .keytip("Q")
                                .icon(IconName::Minus),
                        )
                        .item(
                            RibbonItem::button("app_suite::ZoomActual")
                                .label("100%")
                                .keytip("A")
                                .icon(IconName::Search),
                        )
                        .item(
                            RibbonItem::button("app_suite::ZoomFit")
                                .label("适应窗口")
                                .keytip("F")
                                .icon(IconName::Frame),
                        ),
                )
                .group(
                    RibbonGroup::new("页面")
                        .item(
                            RibbonItem::dropdown("排布")
                                .keytip("P")
                                .icon(IconName::PanelLeft)
                                .item(RibbonDropdownItem::new("app_suite::LayoutSingle"))
                                .item(RibbonDropdownItem::new("app_suite::LayoutTwoUp"))
                                .item(RibbonDropdownItem::new("app_suite::LayoutCover")),
                        )
                        .item(
                            RibbonItem::toggle("app_suite::ToggleMargins")
                                .label("窄边距")
                                .keytip("M")
                                .icon(IconName::ResizeCorner)
                                .checked(self.setup.margins == PageMargins::NARROW),
                        )
                        .item(
                            RibbonItem::toggle("app_suite::ToggleGuides")
                                .label("参考线")
                                .keytip("D")
                                .icon(IconName::Eye)
                                .checked(self.guides),
                        )
                        .item(
                            RibbonItem::toggle("app_suite::CollapseRibbon")
                                .label("折叠功能区")
                                .keytip("R")
                                .icon(IconName::WindowMinimize)
                                .checked(collapsed),
                        ),
                ),
            RibbonTab::new("data", "数据")
                .keytip("K")
                .group(
                    RibbonGroup::new("计算").item(
                        RibbonItem::button("app_suite::Recalc")
                            .label("重新计算")
                            .keytip("L")
                            .icon(IconName::Redo)
                            .large(),
                    ),
                )
                .context(RibbonContext::new("表格工具", rgb(0x0e7490))),
        ]
    }

    /// 应用菜单页签：打开 / 保存 / 打印直接绑命令 id，新建页是自定义内容。
    fn backstage_tabs(&self) -> Vec<BackstageTab> {
        vec![
            BackstageTab::new("new", "新建")
                .icon(IconName::Plus)
                .content(|_window, _cx| {
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_lg().child("新建文档"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x64748b))
                                .child("下面这条走的是与功能区、QAT 完全相同的派发路径。"),
                        )
                        .child(
                            div()
                                .id("suite-new")
                                .w_full()
                                .px(px(12.0))
                                .py(px(8.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_sm()
                                .bg(rgb(0xf1f5f9))
                                .hover(|style| style.bg(rgb(0xe2e8f0)))
                                .child("空白文档（app_suite::New）")
                                .on_click(|_event: &ClickEvent, _window, cx| {
                                    if let Err(error) = cx.dispatch_command("app_suite::New", None)
                                    {
                                        log::warn!("派发命令失败：{error}");
                                    }
                                }),
                        )
                        .into_any_element()
                }),
            BackstageTab::new("open", "打开")
                .icon(IconName::FolderOpen)
                .command("app_suite::Open"),
            BackstageTab::new("save", "保存")
                .icon(IconName::Check)
                .command("app_suite::Save"),
            BackstageTab::new("print", "打印")
                .icon(IconName::File)
                .command("app_suite::Print"),
            BackstageTab::new("options", "选项")
                .icon(IconName::Settings)
                .content(|_window, _cx| {
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_lg().child("选项"))
                        .child(div().text_sm().text_color(rgb(0x64748b)).child(
                            "本演示的开关（窄边距 / 参考线 / 折叠功能区）都做成了命令，\
                                 在视图选项卡里；选项页可承载任意表单。",
                        ))
                        .into_any_element()
                }),
        ]
    }

    /// 正文叠加层：按 `content_rect` 换算到屏幕，字号随缩放同步。
    fn overlays(&self, zoom: f32) -> Vec<rgpui::AnyElement> {
        let layout = self.layout();
        let viewport = self.viewport;
        let font_size = px((15.0 * zoom).max(4.0));
        let title_size = px((22.0 * zoom).max(6.0));
        let weight = if self.bold {
            rgpui::FontWeight::BOLD
        } else {
            rgpui::FontWeight::NORMAL
        };
        let paragraph = match self.style {
            DocStyle::Normal => (font_size, rgb(0x1e293b)),
            DocStyle::Title => (title_size, rgb(0x0f172a)),
            DocStyle::Emphasis => (font_size, rgb(0xb45309)),
        };
        let gap = px((10.0 * zoom).max(2.0));
        let cell = px((12.0 * zoom).max(2.0));

        let mut overlays = Vec::new();
        for (index, page) in self.pages.iter().enumerate().take(layout.page_count()) {
            let content = layout.content_rect(index);
            let origin = viewport.document_to_screen(content.origin);
            let scaled = viewport.document_size_to_screen(content.size);
            let blocks = page.blocks.iter().map(|block| match block {
                Block::Paragraph(text) => div()
                    .w_full()
                    .text_size(paragraph.0)
                    .font_weight(weight)
                    .line_height(relative(1.5))
                    .text_color(paragraph.1)
                    .child(text.clone())
                    .into_any_element(),
                Block::Table { rows, total } => v_flex()
                    .w_full()
                    .border_1()
                    .border_color(rgb(0x94a3b8))
                    .children(rows.iter().map(|(name, value)| {
                        h_flex()
                            .w_full()
                            .justify_between()
                            .px(cell)
                            .text_size(font_size)
                            .text_color(rgb(0x334155))
                            .child(name.clone())
                            .child(format!("{value:.0}"))
                    }))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .px(cell)
                            .font_weight(rgpui::FontWeight::SEMIBOLD)
                            .text_size(font_size)
                            .text_color(rgb(0x0f172a))
                            .bg(rgb(0xf1f5f9))
                            .child("合计")
                            .child(match total {
                                Some(sum) => format!("{sum:.0}"),
                                None => "待重算".to_string(),
                            }),
                    )
                    .into_any_element(),
                Block::Picture(name) => div()
                    .w_full()
                    .py(px(8.0))
                    .text_center()
                    .text_size(font_size)
                    .text_color(rgb(0x475569))
                    .border_1()
                    .border_color(rgb(0xcbd5e1))
                    .bg(rgb(0xe2e8f0))
                    .child(name.clone())
                    .into_any_element(),
            });
            overlays.push(
                v_flex()
                    .absolute()
                    .left(origin.x)
                    .top(origin.y)
                    .w(scaled.width)
                    .h(scaled.height)
                    .gap(gap)
                    .children(blocks)
                    .into_any_element(),
            );
        }
        overlays
    }

    fn on_new(&mut self, _: &New, _window: &mut Window, cx: &mut Context<Self>) {
        self.pages = vec![Page {
            blocks: vec![Block::Paragraph("空白文档".into())],
        }];
        self.style = DocStyle::Normal;
        self.bold = false;
        self.dirty = false;
        self.record("New", cx);
    }

    fn on_open(&mut self, _: &Open, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_block(Block::Paragraph("（已载入示例文档）季度复盘要点".into()));
        self.record("Open", cx);
    }

    fn on_save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
        self.dirty = false;
        self.record("Save", cx);
    }

    fn on_print(&mut self, _: &Print, _window: &mut Window, cx: &mut Context<Self>) {
        let pages = self.pages.len();
        self.last = format!("已提交 {pages} 页到打印队列（{NS}Print）").into();
        cx.notify();
    }

    fn on_paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_block(Block::Paragraph("粘贴自剪贴板的正文段落。".into()));
        self.record("Paste", cx);
    }

    fn on_paste_special(&mut self, _: &PasteSpecial, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_block(Block::Paragraph(
            "选择性粘贴：保留源格式的一段文字。".into(),
        ));
        self.record("PasteSpecial", cx);
    }

    fn on_cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_block(Block::Paragraph("（剪切走了当前段落）".into()));
        self.record("Cut", cx);
    }

    fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        self.record("Copy", cx);
    }

    fn on_bold(&mut self, _: &Bold, _window: &mut Window, cx: &mut Context<Self>) {
        self.bold = !self.bold;
        self.touch();
        self.record("Bold", cx);
    }

    fn on_style_normal(&mut self, _: &StyleNormal, _window: &mut Window, cx: &mut Context<Self>) {
        self.style = DocStyle::Normal;
        self.touch();
        self.record("StyleNormal", cx);
    }

    fn on_style_title(&mut self, _: &StyleTitle, _window: &mut Window, cx: &mut Context<Self>) {
        self.style = DocStyle::Title;
        self.touch();
        self.record("StyleTitle", cx);
    }

    fn on_style_emphasis(
        &mut self,
        _: &StyleEmphasis,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.style = DocStyle::Emphasis;
        self.touch();
        self.record("StyleEmphasis", cx);
    }

    fn on_insert_table(&mut self, _: &InsertTable, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_block(sample_table());
        self.record("InsertTable", cx);
    }

    fn on_insert_picture(
        &mut self,
        _: &InsertPicture,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_block(Block::Picture("示意图 · 季度趋势".into()));
        self.record("InsertPicture", cx);
    }

    fn on_new_page(&mut self, _: &NewPage, _window: &mut Window, cx: &mut Context<Self>) {
        self.pages.push(Page {
            blocks: vec![Block::Paragraph("新的一页".into())],
        });
        self.touch();
        self.record("NewPage", cx);
    }

    fn on_zoom_in(&mut self, _: &ZoomIn, _window: &mut Window, cx: &mut Context<Self>) {
        let anchor = self.anchor;
        self.viewport.zoom_in_at(anchor);
        self.record("ZoomIn", cx);
    }

    fn on_zoom_out(&mut self, _: &ZoomOut, _window: &mut Window, cx: &mut Context<Self>) {
        let anchor = self.anchor;
        self.viewport.zoom_out_at(anchor);
        self.record("ZoomOut", cx);
    }

    fn on_zoom_actual(&mut self, _: &ZoomActual, _window: &mut Window, cx: &mut Context<Self>) {
        let anchor = self.anchor;
        self.viewport.set_zoom_at(1.0, anchor);
        self.record("ZoomActual", cx);
    }

    fn on_zoom_fit(&mut self, _: &ZoomFit, window: &mut Window, cx: &mut Context<Self>) {
        let view = window.bounds().size;
        let document = self.layout().document_size();
        self.viewport.fit(document, view, px(24.0));
        self.record("ZoomFit", cx);
    }

    fn on_layout_single(&mut self, _: &LayoutSingle, _window: &mut Window, cx: &mut Context<Self>) {
        self.arrangement = PageArrangement::Single;
        self.record("LayoutSingle", cx);
    }

    fn on_layout_two_up(&mut self, _: &LayoutTwoUp, _window: &mut Window, cx: &mut Context<Self>) {
        self.arrangement = PageArrangement::TwoUp;
        self.record("LayoutTwoUp", cx);
    }

    fn on_layout_cover(&mut self, _: &LayoutCover, _window: &mut Window, cx: &mut Context<Self>) {
        self.arrangement = PageArrangement::TwoUpCover;
        self.record("LayoutCover", cx);
    }

    fn on_toggle_margins(
        &mut self,
        _: &ToggleMargins,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.setup.margins = if self.setup.margins == PageMargins::NORMAL {
            PageMargins::NARROW
        } else {
            PageMargins::NORMAL
        };
        self.record("ToggleMargins", cx);
    }

    fn on_toggle_guides(&mut self, _: &ToggleGuides, _window: &mut Window, cx: &mut Context<Self>) {
        self.guides = !self.guides;
        self.record("ToggleGuides", cx);
    }

    fn on_collapse_ribbon(
        &mut self,
        _: &CollapseRibbon,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ribbon
            .update(cx, |state, cx| state.toggle_collapsed(cx));
        self.record("CollapseRibbon", cx);
    }

    fn on_recalc(&mut self, _: &Recalc, _window: &mut Window, cx: &mut Context<Self>) {
        let mut filled = 0usize;
        for page in &mut self.pages {
            for block in &mut page.blocks {
                if let Block::Table { rows, total } = block {
                    let sum = rows.iter().map(|(_, value)| *value).sum::<f64>();
                    if total.replace(sum).is_none() {
                        filled += 1;
                    }
                }
            }
        }
        self.last = format!("已重算 {filled} 张表格（{NS}Recalc）").into();
        cx.notify();
    }
}

impl Render for AppSuite {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.dragging {
            self.view_size = window.bounds().size;
        }

        let viewport = self.viewport;
        let zoom = viewport.zoom();
        let page_count = self.pages.len();
        let overlays = self.overlays(zoom);
        let last = self.last.clone();
        let dirty = if self.dirty { "未保存" } else { "已保存" };
        let margins = if self.setup.margins == PageMargins::NARROW {
            "窄边距"
        } else {
            "常规边距"
        };
        let stale = self.stale_tables();
        let arrangement = self.arrangement_label();
        let collapsed = self.ribbon.read(cx).is_collapsed();
        let tabs = self.ribbon_tabs(collapsed);
        let backstage_tabs = self.backstage_tabs();

        v_flex()
            .id("app-suite")
            .key_context("Ribbon")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .bg(rgb(0xffffff))
            .on_action(cx.listener(Self::on_new))
            .on_action(cx.listener(Self::on_open))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_print))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_paste_special))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_bold))
            .on_action(cx.listener(Self::on_style_normal))
            .on_action(cx.listener(Self::on_style_title))
            .on_action(cx.listener(Self::on_style_emphasis))
            .on_action(cx.listener(Self::on_insert_table))
            .on_action(cx.listener(Self::on_insert_picture))
            .on_action(cx.listener(Self::on_new_page))
            .on_action(cx.listener(Self::on_zoom_in))
            .on_action(cx.listener(Self::on_zoom_out))
            .on_action(cx.listener(Self::on_zoom_actual))
            .on_action(cx.listener(Self::on_zoom_fit))
            .on_action(cx.listener(Self::on_layout_single))
            .on_action(cx.listener(Self::on_layout_two_up))
            .on_action(cx.listener(Self::on_layout_cover))
            .on_action(cx.listener(Self::on_toggle_margins))
            .on_action(cx.listener(Self::on_toggle_guides))
            .on_action(cx.listener(Self::on_collapse_ribbon))
            .on_action(cx.listener(Self::on_recalc))
            // 顶栏：文件入口 + 快速访问工具栏 + 保存状态。
            .child(
                h_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(0x1e293b))
                    .child(
                        div()
                            .id("file-entry")
                            .px(px(12.0))
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(rgb(0xf8fafc))
                            .bg(rgb(0x334155))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.toggle_backstage(window, cx);
                            }))
                            .child("文件"),
                    )
                    .child(
                        QuickAccessToolbar::new()
                            .item(QatItem::button("app_suite::New").icon(IconName::Plus))
                            .item(QatItem::button("app_suite::Open").icon(IconName::FolderOpen))
                            .item(QatItem::button("app_suite::Save").icon(IconName::Check))
                            .item(QatItem::separator())
                            .item(QatItem::button("app_suite::Cut").icon(IconName::Delete))
                            .item(QatItem::button("app_suite::Copy").icon(IconName::Copy))
                            .item(QatItem::button("app_suite::Paste").label("粘贴")),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .text_xs()
                            .text_color(rgb(0xcbd5e1))
                            .child(format!("{dirty} · {page_count} 页")),
                    ),
            )
            .child(Ribbon::new(&self.ribbon, tabs))
            // 内容区：分页画布 + 视口（滚轮 Ctrl/⌘ 缩放，否则平移，左键拖拽平移）。
            .child(
                div()
                    .relative()
                    .flex_grow_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .bg(rgb(0xf1f5f9))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.dragging = true;
                            this.last_pointer = event.position;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.dragging = false;
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.anchor = event.position;
                        if this.dragging {
                            let delta = event.position - this.last_pointer;
                            this.last_pointer = event.position;
                            this.viewport.pan_by(delta);
                        }
                        cx.notify();
                    }))
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        this.viewport.apply_wheel(event, px(50.0));
                        cx.notify();
                    }))
                    .child(
                        div().absolute().top_0().left_0().size_full().child(
                            PageView::new(
                                self.layout(),
                                viewport,
                                SuiteLabels {
                                    title: "rgpui 应用套件基座".into(),
                                    pages: page_count,
                                },
                            )
                            .content_guide(self.guides)
                            .area_guide(self.guides),
                        ),
                    )
                    .children(overlays),
            )
            // 状态栏：最近命令、命令清单规模、待重算表格、视口读数。
            .child(
                h_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .border_t_1()
                    .border_color(rgb(0xcbd5e1))
                    .bg(rgb(0xf8fafc))
                    .text_xs()
                    .text_color(rgb(0x475569))
                    .child(format!("最近命令：{last}"))
                    .child(format!(
                        "命令清单 {} 条 · 待重算 {stale} 张 · 排布 {arrangement} · {margins}",
                        CommandRegistry::global().len()
                    ))
                    .child(div().ml_auto().child(format!("缩放 {:.0}%", zoom * 100.0))),
            )
            .child(Backstage::new(&self.backstage, backstage_tabs).sidebar_width(px(150.0)))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        // 快捷键与注册表同源：描述符里的 `with_keystroke` 在这里逐条落到键映射上。
        cx.bind_keys([
            KeyBinding::new("ctrl-n", New, None),
            KeyBinding::new("ctrl-o", Open, None),
            KeyBinding::new("ctrl-s", Save, None),
            KeyBinding::new("ctrl-p", Print, None),
            KeyBinding::new("ctrl-v", Paste, None),
            KeyBinding::new("ctrl-x", Cut, None),
            KeyBinding::new("ctrl-c", Copy, None),
            KeyBinding::new("ctrl-b", Bold, None),
            KeyBinding::new("f9", Recalc, None),
        ]);
        let bounds = Bounds::centered(None, size(px(1040.0), px(780.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| AppSuite::new(window, cx)),
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

#[cfg(test)]
mod tests {
    use rgpui::CommandRegistry;

    /// gate：本示例登记的命令清单自洽（id 唯一 / 标签与分类非空 / 快捷键可解析）。
    #[test]
    fn registered_commands_are_valid() {
        let problems = CommandRegistry::global().validate();
        assert!(problems.is_empty(), "命令清单存在问题：{problems:?}");
    }
}
