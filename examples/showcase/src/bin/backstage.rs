//! 应用菜单（Backstage）演示：点击左上「文件」展开覆盖整窗的面板，
//! 左侧是页面导航（新建 / 打开 / 保存 / 打印 / 选项 / 关于），右侧随页面
//! 切换为自定义内容或一条命令。Esc 或「返回」关闭。
//!
//! 对应 `rgpui` 的 `ribbon` feature（`rgpui::backstage`）。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::backstage::{Backstage, BackstageState, BackstageTab};
use rgpui::{
    App, Bounds, ClickEvent, CommandDescriptor, Context, FocusHandle, IconName,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, WindowBounds, WindowOptions, actions,
    div, prelude::*, px, register_command, rgb, size, v_flex,
};
use rgpui_platform::application;

actions!(backstage_demo, [New, Open, Save, Print, Options]);

/// 「新建」命令。
fn cmd_new() -> CommandDescriptor {
    CommandDescriptor::new("backstage::New", "新建").with_category("文件")
}

/// 「打开」命令。
fn cmd_open() -> CommandDescriptor {
    CommandDescriptor::new("backstage::Open", "打开")
        .with_category("文件")
        .with_keystroke("ctrl-o")
}

/// 「保存」命令。
fn cmd_save() -> CommandDescriptor {
    CommandDescriptor::new("backstage::Save", "保存")
        .with_category("文件")
        .with_keystroke("ctrl-s")
}

/// 「打印」命令。
fn cmd_print() -> CommandDescriptor {
    CommandDescriptor::new("backstage::Print", "打印").with_category("文件")
}

/// 「选项」命令。
fn cmd_options() -> CommandDescriptor {
    CommandDescriptor::new("backstage::Options", "选项").with_category("文件")
}

register_command!(cmd_new);
register_command!(cmd_open);
register_command!(cmd_save);
register_command!(cmd_print);
register_command!(cmd_options);

/// 演示视图：顶栏「文件」按钮 + 内容区 + 应用菜单覆盖层。
struct BackstageDemo {
    state: rgpui::Entity<BackstageState>,
    focus_handle: FocusHandle,
    last: String,
    template: Option<SharedString>,
}

impl BackstageDemo {
    /// 创建视图。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(BackstageState::new);
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            state,
            focus_handle,
            last: "（尚未执行命令）".to_string(),
            template: None,
        }
    }

    /// 记录最近执行的命令。
    fn record(&mut self, name: &str, cx: &mut Context<Self>) {
        self.last = name.to_string();
        cx.notify();
    }

    /// 选择模板（来自「新建」页的自定义内容）。
    fn pick_template(&mut self, template: &str, cx: &mut Context<Self>) {
        let template: SharedString = template.into();
        self.template = Some(template.clone());
        self.record(&format!("新建自模板「{template}」"), cx);
    }

    /// 打开/关闭应用菜单，并把焦点交给面板（这样 Esc 才生效）。
    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = !self.state.read(cx).is_open();
        self.state.update(cx, |state, cx| {
            if open {
                state.open(cx);
            } else {
                state.close(cx);
            }
        });
        let focus_handle = self.state.read(cx).focus_handle();
        window.focus(&focus_handle, cx);
    }
}

impl Render for BackstageDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 「新建」页的自定义内容需要回写本视图，用弱句柄避免循环引用。
        let weak = cx.entity().downgrade();
        let template = self.template.clone();
        let last = self.last.clone();

        let tabs = vec![
            BackstageTab::new("new", "新建")
                .icon(IconName::Plus)
                .content({
                    let weak = weak.clone();
                    move |_window, _cx| {
                        let weak = weak.clone();
                        v_flex()
                            .gap(px(8.0))
                            .child(div().text_lg().child("从模板新建"))
                            .children(["空白文档", "报表模板", "会议纪要"].map(|name| {
                                let weak = weak.clone();
                                div()
                                    .id(name)
                                    .px(px(12.0))
                                    .py(px(8.0))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_sm()
                                    .bg(rgb(0xf1f5f9))
                                    .hover(|style| style.bg(rgb(0xe2e8f0)))
                                    .child(name)
                                    .on_click(move |_event: &ClickEvent, _window, cx| {
                                        _ = weak.update(cx, |this, cx| {
                                            this.pick_template(name, cx);
                                        });
                                    })
                            }))
                            .into_any_element()
                    }
                }),
            BackstageTab::new("open", "打开")
                .icon(IconName::FolderOpen)
                .command("backstage::Open"),
            BackstageTab::new("save", "保存")
                .icon(IconName::Check)
                .command("backstage::Save"),
            BackstageTab::new("print", "打印")
                .icon(IconName::File)
                .command("backstage::Print"),
            BackstageTab::new("options", "选项")
                .icon(IconName::Settings)
                .content(|_window, _cx| {
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_lg().child("选项"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x64748b))
                                .child("设置页可承载任意表单，这里只演示内容页占位。"),
                        )
                        .into_any_element()
                }),
            BackstageTab::new("about", "关于")
                .icon(IconName::Search)
                .content(|_window, _cx| {
                    v_flex()
                        .gap(px(8.0))
                        .child(div().text_lg().child("关于"))
                        .child(div().text_sm().child("应用菜单（Backstage）演示 · rgpui"))
                        .into_any_element()
                }),
        ];

        let template_line = match &template {
            Some(name) => format!("当前模板：{name}"),
            None => "当前模板：未选择".to_string(),
        };

        v_flex()
            .id("backstage-demo")
            .relative()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &New, _, cx| this.record("新建", cx)))
            .on_action(cx.listener(|this, _: &Open, _, cx| this.record("打开", cx)))
            .on_action(cx.listener(|this, _: &Save, _, cx| this.record("保存", cx)))
            .on_action(cx.listener(|this, _: &Print, _, cx| this.record("打印", cx)))
            .on_action(cx.listener(|this, _: &Options, _, cx| this.record("选项", cx)))
            .size_full()
            .bg(rgb(0xffffff))
            // 顶栏：「文件」按钮即应用菜单入口。
            .child(
                div()
                    .id("file-button")
                    .w_full()
                    .flex_shrink_0()
                    .px(px(16.0))
                    .py(px(8.0))
                    .bg(rgb(0x1e293b))
                    .text_color(rgb(0xf8fafc))
                    .text_sm()
                    .cursor_pointer()
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.toggle(window, cx)),
                    )
                    .child("文件"),
            )
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(16.0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(rgpui::FontWeight::BOLD)
                            .child("应用菜单（Backstage）demo"),
                    )
                    .child(div().text_sm().child(format!("最近操作：{last}")))
                    .child(div().text_sm().child(template_line))
                    .child(div().text_xs().text_color(rgb(0x64748b)).child(
                        "点击左上「文件」展开面板：左侧切换页面，右侧是内容页或一条命令；\
                         关闭可通过「返回」或 Esc。",
                    )),
            )
            .child(Backstage::new(&self.state, tabs))
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(720.0), px(440.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| BackstageDemo::new(window, cx)),
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
