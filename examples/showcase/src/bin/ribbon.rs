//! Ribbon 功能区演示：数据驱动的选项卡/分组/大按钮/小按钮/切换/下拉，
//! 命令按 id 绑定到 `CommandRegistry`，点击经 `App::dispatch_command` 派发到 `on_action`。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::ribbon::{
    Ribbon, RibbonComboBoxItem, RibbonContext, RibbonDropdownItem, RibbonGalleryItem, RibbonGroup,
    RibbonItem, RibbonState, RibbonTab,
};
use rgpui::{
    App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, Render, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, register_command, rgb, size, v_flex,
};
use rgpui_platform::application;

actions!(
    demo,
    [
        Paste,
        PasteSpecial,
        Cut,
        Copy,
        Bold,
        InsertTable,
        InsertPicture,
        InsertChart,
        StyleNormal,
        StyleTitle,
        StyleEmphasis,
        Settings
    ]
);

/// 「粘贴」命令。
fn cmd_paste() -> CommandDescriptor {
    CommandDescriptor::new("demo::Paste", "粘贴")
        .with_category("剪贴板")
        .with_keystroke("ctrl-v")
        .with_keytip("V")
}
/// 「选择性粘贴」命令（粘贴分裂按钮的下拉项）。
fn cmd_paste_special() -> CommandDescriptor {
    CommandDescriptor::new("demo::PasteSpecial", "选择性粘贴").with_category("剪贴板")
}
/// 「剪切」命令。
fn cmd_cut() -> CommandDescriptor {
    CommandDescriptor::new("demo::Cut", "剪切")
        .with_category("剪贴板")
        .with_keytip("X")
}
/// 「复制」命令。
fn cmd_copy() -> CommandDescriptor {
    CommandDescriptor::new("demo::Copy", "复制")
        .with_category("剪贴板")
        .with_keytip("C")
}
/// 「加粗」命令。
fn cmd_bold() -> CommandDescriptor {
    CommandDescriptor::new("demo::Bold", "加粗")
        .with_category("字体")
        .with_keytip("B")
}
/// 「常规样式」命令（样式库项）。
fn cmd_style_normal() -> CommandDescriptor {
    CommandDescriptor::new("demo::StyleNormal", "常规").with_category("样式")
}
/// 「标题样式」命令（样式库项）。
fn cmd_style_title() -> CommandDescriptor {
    CommandDescriptor::new("demo::StyleTitle", "标题").with_category("样式")
}
/// 「强调样式」命令（样式库项）。
fn cmd_style_emphasis() -> CommandDescriptor {
    CommandDescriptor::new("demo::StyleEmphasis", "强调").with_category("样式")
}
/// 「插入表格」命令。
fn cmd_insert_table() -> CommandDescriptor {
    CommandDescriptor::new("demo::InsertTable", "表格")
        .with_category("插入")
        .with_keytip("T")
}
/// 「插入图片」命令。
fn cmd_insert_picture() -> CommandDescriptor {
    CommandDescriptor::new("demo::InsertPicture", "图片")
        .with_category("插入")
        .with_keytip("P")
}
/// 「插入图表」命令（插入组合框项）。
fn cmd_insert_chart() -> CommandDescriptor {
    CommandDescriptor::new("demo::InsertChart", "图表").with_category("插入")
}
/// 「设置」命令。
fn cmd_settings() -> CommandDescriptor {
    CommandDescriptor::new("demo::Settings", "设置")
        .with_category("文件")
        .with_keytip("S")
}

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
register_command!(cmd_insert_chart);
register_command!(cmd_settings);

/// 演示视图。
struct RibbonDemo {
    state: rgpui::Entity<RibbonState>,
    focus_handle: FocusHandle,
    last: String,
    bold: bool,
}

impl RibbonDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(RibbonState::new);
        // 切换选项卡时重绘本视图。
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            state,
            focus_handle,
            last: "（尚未执行命令）".to_string(),
            bold: false,
        }
    }

    fn record(&mut self, name: &str, cx: &mut Context<Self>) {
        self.last = name.to_string();
        cx.notify();
    }
}

impl Render for RibbonDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = vec![
            RibbonTab::new("home", "开始")
                .keytip("H")
                .group(
                    RibbonGroup::new("剪贴板")
                        .item(
                            RibbonItem::split("demo::Paste")
                                .label("粘贴")
                                .keytip("V")
                                .icon(IconName::Copy)
                                .large()
                                .item(
                                    RibbonDropdownItem::new("demo::PasteSpecial")
                                        .label("选择性粘贴"),
                                ),
                        )
                        .item(
                            RibbonItem::button("demo::Cut")
                                .label("剪切")
                                .icon(IconName::Delete),
                        )
                        .item(
                            RibbonItem::button("demo::Copy")
                                .label("复制")
                                .icon(IconName::Copy),
                        ),
                )
                .group(
                    RibbonGroup::new("插入")
                        .item(
                            RibbonItem::button("demo::InsertTable")
                                .label("表格")
                                .icon(IconName::LayoutDashboard)
                                .large(),
                        )
                        .item(
                            RibbonItem::dropdown("更多")
                                .keytip("M")
                                .icon(IconName::Plus)
                                .item(RibbonDropdownItem::new("demo::InsertPicture"))
                                .item(RibbonDropdownItem::new("demo::Settings")),
                        )
                        .item(
                            RibbonItem::combobox("insert-kind")
                                .keytip("K")
                                .width(96.0)
                                .selected(0)
                                .placeholder("插入")
                                .item(RibbonComboBoxItem::new("demo::InsertTable").label("表格"))
                                .item(RibbonComboBoxItem::new("demo::InsertPicture").label("图片"))
                                .item(RibbonComboBoxItem::new("demo::InsertChart").label("图表")),
                        ),
                )
                .group(
                    RibbonGroup::new("样式")
                        .item(
                            RibbonItem::gallery("样式库")
                                .keytip("G")
                                .columns(2)
                                .large()
                                .icon(IconName::Palette)
                                .item(
                                    RibbonGalleryItem::new("demo::StyleNormal")
                                        .label("常规")
                                        .icon(IconName::File)
                                        .selected(true),
                                )
                                .item(
                                    RibbonGalleryItem::new("demo::StyleTitle")
                                        .label("标题")
                                        .icon(IconName::Star),
                                )
                                .item(
                                    RibbonGalleryItem::new("demo::StyleEmphasis")
                                        .label("强调")
                                        .icon(IconName::Palette),
                                ),
                        )
                        .item(
                            RibbonItem::toggle("demo::Bold")
                                .label("加粗")
                                .icon(IconName::Palette)
                                .checked(self.bold),
                        ),
                ),
            RibbonTab::new("insert", "插入")
                .keytip("N")
                .group(
                    RibbonGroup::new("插图").item(
                        RibbonItem::button("demo::InsertPicture")
                            .label("图片")
                            .icon(IconName::GalleryVerticalEnd)
                            .large(),
                    ),
                )
                .context(RibbonContext::new("图片工具", rgb(0xc2410c))),
        ];

        v_flex()
            .id("ribbon-demo")
            .key_context("Ribbon")
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|this, _: &rgpui::ribbon::ToggleKeyTips, window, cx| {
                    this.state
                        .update(cx, |state, cx| state.toggle_keytips(window, cx));
                }),
            )
            .on_action(cx.listener(|this, _: &Paste, _, cx| this.record("粘贴", cx)))
            .on_action(cx.listener(|this, _: &PasteSpecial, _, cx| this.record("选择性粘贴", cx)))
            .on_action(cx.listener(|this, _: &Cut, _, cx| this.record("剪切", cx)))
            .on_action(cx.listener(|this, _: &Copy, _, cx| this.record("复制", cx)))
            .on_action(cx.listener(|this, _: &Bold, _, cx| {
                let bold = !this.bold;
                this.bold = bold;
                this.record("加粗", cx);
            }))
            .on_action(cx.listener(|this, _: &StyleNormal, _, cx| this.record("常规样式", cx)))
            .on_action(cx.listener(|this, _: &StyleTitle, _, cx| this.record("标题样式", cx)))
            .on_action(cx.listener(|this, _: &StyleEmphasis, _, cx| this.record("强调样式", cx)))
            .on_action(cx.listener(|this, _: &InsertTable, _, cx| this.record("插入表格", cx)))
            .on_action(cx.listener(|this, _: &InsertPicture, _, cx| this.record("插入图片", cx)))
            .on_action(cx.listener(|this, _: &InsertChart, _, cx| this.record("插入图表", cx)))
            .on_action(cx.listener(|this, _: &Settings, _, cx| this.record("设置", cx)))
            .size_full()
            .bg(rgpui::rgb(0xffffff))
            .child(Ribbon::new(&self.state, tabs))
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(16.0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("Ribbon 功能区 demo"),
                    )
                    .child(div().text_sm().child(format!("最近执行：{}", self.last)))
                    .child(div().text_xs().text_color(rgpui::rgb(0x64748b)).child(
                        "点击功能区按钮或切换选项卡；命令来自 CommandRegistry，按 id 派发。\
                         按 Alt/F10 显示按键提示，再按选项卡/命令提示键即可键盘操作。",
                    )),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(820.0), px(420.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| RibbonDemo::new(window, cx)),
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
