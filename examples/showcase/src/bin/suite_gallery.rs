//! 应用套件 示例集合。
//!
//! 用法：cargo run -p showcase --bin suite_gallery -- <demo>
//! 可用 demo：backstage / command_registry / mini_toolbar / qat / ribbon / canvas_objects / ruler / viewport / formula_bar / sheet_grid / app_suite / automation
//! 不带参数时默认运行 command_registry，传 --list 打印清单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    AnyView, App, Bounds, Button, ButtonVariants, Context, IntoElement, ParentElement, Render,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

mod backstage {
    //! 应用菜单（Backstage）演示：点击左上「文件」展开覆盖整窗的面板，
    //! 左侧是页面导航（新建 / 打开 / 保存 / 打印 / 选项 / 关于），右侧随页面
    //! 切换为自定义内容或一条命令。Esc 或「返回」关闭。
    //!
    //! 对应 `rgpui` 的 `ribbon` feature（`rgpui::backstage`）。

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

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| BackstageDemo::new(window, cx)).into()
    }
}

mod command_registry {
    //! command registry 演示：`register_command!` 登记命令元数据 → `CommandRegistry` 读取 →
    //! 点击按 id 派发（`App::dispatch_command`）→ 视图 `on_action` 处理。
    //!
    //! 展示了套件类应用「一切皆命令」的骨架：UI、快捷键、命令面板、CLI、MCP 共用同一份
    //! 命令表，按 id 分发，与具体前端解耦。

    use rgpui::{
        App, Bounds, CommandDescriptor, CommandRegistry, Context, FocusHandle, Render, Window,
        WindowBounds, WindowOptions, actions, div, h_flex, prelude::*, px, register_command, size,
        v_flex,
    };
    use rgpui_platform::application;

    actions!(command_registry_demo, [Increment, Decrement, Reset]);

    /// 「增加」命令的描述符。
    fn cmd_increment() -> CommandDescriptor {
        CommandDescriptor::new("demo::Increment", "增加")
            .with_category("计数")
            .with_description("把计数加一")
            .with_keystroke("ctrl-i")
    }

    /// 「减少」命令的描述符。
    fn cmd_decrement() -> CommandDescriptor {
        CommandDescriptor::new("demo::Decrement", "减少")
            .with_category("计数")
            .with_description("把计数减一")
            .with_keystroke("ctrl-d")
    }

    /// 「重置」命令的描述符。
    fn cmd_reset() -> CommandDescriptor {
        CommandDescriptor::new("demo::Reset", "重置")
            .with_category("计数")
            .with_description("把计数清零")
    }

    register_command!(cmd_increment);
    register_command!(cmd_decrement);
    register_command!(cmd_reset);

    /// 演示视图：显示计数，并列出注册表中的全部命令。
    struct CommandRegistryDemo {
        count: i32,
        focus_handle: FocusHandle,
    }

    impl CommandRegistryDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                count: 0,
                focus_handle,
            }
        }

        fn on_increment(&mut self, _: &Increment, _window: &mut Window, cx: &mut Context<Self>) {
            self.count += 1;
            cx.notify();
        }

        fn on_decrement(&mut self, _: &Decrement, _window: &mut Window, cx: &mut Context<Self>) {
            self.count -= 1;
            cx.notify();
        }

        fn on_reset(&mut self, _: &Reset, _window: &mut Window, cx: &mut Context<Self>) {
            self.count = 0;
            cx.notify();
        }
    }

    impl Render for CommandRegistryDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let registry = CommandRegistry::global();

            let rows = registry.iter().map(|descriptor| {
                let id = descriptor.id().clone();
                let shortcut = descriptor
                    .default_keystrokes()
                    .first()
                    .map(|key| key.to_string())
                    .unwrap_or_default();
                let label = format!("{}（{}）{}", descriptor.label(), descriptor.id(), shortcut);
                div()
                    .id(descriptor.id().clone())
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded_md()
                    .bg(rgpui::rgb(0xeef2ff))
                    .cursor_pointer()
                    .text_sm()
                    .hover(|style| style.bg(rgpui::rgb(0xdfe6ff)))
                    .child(label)
                    .on_click(move |_, _window, cx| {
                        if let Err(error) = cx.dispatch_command(&id, None) {
                            log::warn!("派发命令 `{id}` 失败：{error}");
                        }
                    })
            });

            let summary = format!(
                "计数：{}  ｜  已登记 {} 条命令，分类：{}",
                self.count,
                registry.len(),
                registry
                    .categories()
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" / ")
            );

            v_flex()
                .id("command-registry-demo")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_increment))
                .on_action(cx.listener(Self::on_decrement))
                .on_action(cx.listener(Self::on_reset))
                .size_full()
                .gap(px(10.0))
                .p(px(16.0))
                .bg(rgpui::rgb(0xf8fafc))
                .child(
                    div()
                        .text_lg()
                        .font_weight(rgpui::FontWeight::BOLD)
                        .child("命令注册表 demo"),
                )
                .child(div().text_sm().child(summary))
                .child(
                    div().text_xs().text_color(rgpui::rgb(0x64748b)).child(
                        "点击任意命令按 id 派发；也可用 Ctrl+I / Ctrl+D（需先加载快捷键）。",
                    ),
                )
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(
                            div()
                                .id("cmd-increment")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("增加（demo::Increment）")
                                .on_click(|_, _window, cx| {
                                    let _ = cx.dispatch_command("demo::Increment", None);
                                }),
                        )
                        .child(
                            div()
                                .id("cmd-decrement")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("减少（demo::Decrement）")
                                .on_click(|_, _window, cx| {
                                    let _ = cx.dispatch_command("demo::Decrement", None);
                                }),
                        ),
                )
                .child(
                    v_flex()
                        .gap(px(4.0))
                        .flex_1()
                        .overflow_hidden()
                        .children(rows),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(560.0), px(480.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| CommandRegistryDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| CommandRegistryDemo::new(window, cx)).into()
    }
}

mod mini_toolbar {
    //! 迷你工具栏（MiniToolbar）演示：在「文档」上拖动选择后就近浮出紧凑命令条，
    //! 按钮与 QAT 同源（`QatItem`，按命令 id 派发）。
    //!
    //! 对应 `rgpui` 的 `ribbon` feature（`rgpui::mini_toolbar` + `rgpui::qat`）。

    use rgpui::mini_toolbar::MiniToolbar;
    use rgpui::qat::QatItem;
    use rgpui::{
        App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, MouseButton, Pixels,
        Render, SharedString, Window, WindowBounds, WindowOptions, actions, div, point, prelude::*,
        px, register_command, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 页面顶部说明条高度（此区域不参与选择）。
    const HEADER_H: Pixels = px(56.0);

    actions!(
        mini_toolbar_minitoolbar,
        [Bold, Italic, Underline, Highlight, Copy]
    );

    /// 「加粗」命令描述符。
    fn cmd_bold() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Bold", "加粗")
            .with_category("格式")
            .with_description("切换选区加粗")
    }

    /// 「斜体」命令描述符。
    fn cmd_italic() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Italic", "斜体").with_category("格式")
    }

    /// 「下划线」命令描述符。
    fn cmd_underline() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Underline", "下划线").with_category("格式")
    }

    /// 「高亮」命令描述符。
    fn cmd_highlight() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Highlight", "高亮").with_category("格式")
    }

    /// 「复制」命令描述符。
    fn cmd_copy() -> CommandDescriptor {
        CommandDescriptor::new("minitoolbar::Copy", "复制")
            .with_category("剪贴板")
            .with_keystroke("ctrl-c")
    }

    register_command!(cmd_bold);
    register_command!(cmd_italic);
    register_command!(cmd_underline);
    register_command!(cmd_highlight);
    register_command!(cmd_copy);

    /// 选区（起点 → 终点，窗口坐标）。
    type Selection = (rgpui::Point<Pixels>, rgpui::Point<Pixels>);

    /// 演示视图：假文档 + 拖动选择 + 就近浮出的迷你工具栏。
    struct MiniToolbarDemo {
        focus_handle: FocusHandle,
        dragging: bool,
        selection: Option<Selection>,
        bold: bool,
        italic: bool,
        underline: bool,
        highlight: bool,
        status: SharedString,
    }

    impl MiniToolbarDemo {
        /// 创建视图并聚焦自身。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                focus_handle,
                dragging: false,
                selection: None,
                bold: false,
                italic: false,
                underline: false,
                highlight: false,
                status: SharedString::from("在下方文档区按住左键拖动即可选中。"),
            }
        }

        /// 设置状态栏文本。
        fn set_status(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
            self.status = text.into();
            cx.notify();
        }

        fn on_bold(&mut self, _: &Bold, _window: &mut Window, cx: &mut Context<Self>) {
            self.bold = !self.bold;
            let state = if self.bold { "开" } else { "关" };
            self.set_status(format!("加粗：{state}"), cx);
        }

        fn on_italic(&mut self, _: &Italic, _window: &mut Window, cx: &mut Context<Self>) {
            self.italic = !self.italic;
            let state = if self.italic { "开" } else { "关" };
            self.set_status(format!("斜体：{state}"), cx);
        }

        fn on_underline(&mut self, _: &Underline, _window: &mut Window, cx: &mut Context<Self>) {
            self.underline = !self.underline;
            let state = if self.underline { "开" } else { "关" };
            self.set_status(format!("下划线：{state}"), cx);
        }

        fn on_highlight(&mut self, _: &Highlight, _window: &mut Window, cx: &mut Context<Self>) {
            self.highlight = !self.highlight;
            let state = if self.highlight { "开" } else { "关" };
            self.set_status(format!("高亮：{state}"), cx);
        }

        fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
            self.set_status("已复制选区（演示）", cx);
        }

        /// 当前格式状态的文字摘要。
        fn format_summary(&self) -> String {
            let mark = |on: bool| if on { "开" } else { "关" };
            format!(
                "加粗 {} ｜ 斜体 {} ｜ 下划线 {} ｜ 高亮 {}",
                mark(self.bold),
                mark(self.italic),
                mark(self.underline),
                mark(self.highlight)
            )
        }

        /// 迷你工具栏的落点：选区终点右下偏移，避免遮住选区。
        fn toolbar_at(selection: Selection) -> rgpui::Point<Pixels> {
            point(selection.1.x + px(4.0), selection.1.y + px(18.0))
        }
    }

    impl Render for MiniToolbarDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let selection = self.selection;
            let summary = self.format_summary();
            let status = self.status.clone();

            // 选区高亮矩形（根元素原点即窗口原点，坐标可直接用）。
            let selection_rect = selection.map(|(a, b)| {
                let origin = point(a.x.min(b.x), a.y.min(b.y));
                let extent = size((a.x - b.x).abs(), (a.y - b.y).abs());
                div()
                    .absolute()
                    .left(origin.x)
                    .top(origin.y)
                    .w(extent.width)
                    .h(extent.height)
                    .bg(rgb(0x60a5fa).opacity(0.25))
            });

            div()
                .id("mini-toolbar-demo")
                .relative()
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_bold))
                .on_action(cx.listener(Self::on_italic))
                .on_action(cx.listener(Self::on_underline))
                .on_action(cx.listener(Self::on_highlight))
                .on_action(cx.listener(Self::on_copy))
                .on_key_down(
                    cx.listener(|this, event: &rgpui::KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" && this.selection.take().is_some() {
                            cx.notify();
                        }
                    }),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &rgpui::MouseDownEvent, _window, cx| {
                        if event.position.y < HEADER_H {
                            return;
                        }
                        this.dragging = true;
                        this.selection = Some((event.position, event.position));
                        cx.notify();
                    }),
                )
                .on_mouse_move(
                    cx.listener(|this, event: &rgpui::MouseMoveEvent, _window, cx| {
                        if let Some((start, _)) = this.selection
                            && this.dragging
                        {
                            this.selection = Some((start, event.position));
                            cx.notify();
                        }
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _event: &rgpui::MouseUpEvent, _window, cx| {
                        this.dragging = false;
                        cx.notify();
                    }),
                )
                .size_full()
                .bg(rgb(0xf8fafc))
                // 说明条（覆盖在顶部，不参与选择）。
                .child(
                    v_flex()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(HEADER_H)
                        .justify_center()
                        .gap(px(2.0))
                        .px(px(16.0))
                        .bg(rgb(0xf1f5f9))
                        .border_b_1()
                        .border_color(rgb(0xe2e8f0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(rgpui::FontWeight::BOLD)
                                .child("迷你工具栏（MiniToolbar）demo"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child("在文档区按住左键拖动 → 就近浮出命令条；Esc 取消选择。"),
                        ),
                )
                // 假文档内容。
                .child(
                    v_flex()
                        .absolute()
                        .top(HEADER_H)
                        .left_0()
                        .w_full()
                        .p(px(24.0))
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x334155))
                                .child("rgpui 的迷你工具栏与快速访问工具栏共用 QatItem："),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x334155))
                                .child("按钮按命令 id 绑定，点击经 App::dispatch_command 派发。"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x334155))
                                .child("选择这段文字试试：松开鼠标后面板会停在选区旁边。"),
                        ),
                )
                // 选区高亮。
                .children(selection_rect)
                // 迷你工具栏（就近浮出）。
                .children(selection.map(|selection| {
                    MiniToolbar::new()
                        .at(Self::toolbar_at(selection))
                        .item(QatItem::button("minitoolbar::Bold").label("B"))
                        .item(QatItem::button("minitoolbar::Italic").label("I"))
                        .item(QatItem::button("minitoolbar::Underline").label("U"))
                        .item(QatItem::separator())
                        .item(QatItem::button("minitoolbar::Highlight").icon(IconName::Palette))
                        .item(QatItem::button("minitoolbar::Copy").icon(IconName::Copy))
                }))
                // 底部状态条。
                .child(
                    v_flex()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .w_full()
                        .gap(px(2.0))
                        .px(px(16.0))
                        .py(px(6.0))
                        .bg(rgb(0xf1f5f9))
                        .border_t_1()
                        .border_color(rgb(0xe2e8f0))
                        .child(div().text_xs().child(status))
                        .child(div().text_xs().text_color(rgb(0x64748b)).child(summary)),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| MiniToolbarDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| MiniToolbarDemo::new(window, cx)).into()
    }
}

mod qat {
    //! 快速访问工具栏（QAT）演示：一排按**命令 id** 绑定的紧凑图标按钮，
    //! 通常置于 Ribbon / 标题栏之上；点击经 `App::dispatch_command` 派发，
    //! 与 Ribbon、菜单、命令面板共用同一份命令表。
    //!
    //! 本例对应 `rgpui` 的 `ribbon` feature（`QatItem` / `QuickAccessToolbar`）。

    use rgpui::qat::{QatItem, QuickAccessToolbar};
    use rgpui::{
        App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, Render, SharedString,
        Window, WindowBounds, WindowOptions, actions, div, h_flex, prelude::*, px,
        register_command, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    actions!(qat_qat, [Save, Undo, Redo, Settings, Search]);

    /// 「保存」命令描述符。
    fn cmd_save() -> CommandDescriptor {
        CommandDescriptor::new("qat::Save", "保存")
            .with_category("快速访问")
            .with_keystroke("ctrl-s")
    }

    /// 「撤销」命令描述符。
    fn cmd_undo() -> CommandDescriptor {
        CommandDescriptor::new("qat::Undo", "撤销")
            .with_category("快速访问")
            .with_keystroke("ctrl-z")
    }

    /// 「重做」命令描述符。
    fn cmd_redo() -> CommandDescriptor {
        CommandDescriptor::new("qat::Redo", "重做")
            .with_category("快速访问")
            .with_keystroke("ctrl-y")
    }

    /// 「设置」命令描述符。
    fn cmd_settings() -> CommandDescriptor {
        CommandDescriptor::new("qat::Settings", "设置")
            .with_category("快速访问")
            .with_description("打开应用设置")
    }

    /// 「查找」命令描述符。
    fn cmd_search() -> CommandDescriptor {
        CommandDescriptor::new("qat::Search", "查找").with_category("快速访问")
    }

    register_command!(cmd_save);
    register_command!(cmd_undo);
    register_command!(cmd_redo);
    register_command!(cmd_settings);
    register_command!(cmd_search);

    /// 演示视图：顶部一条 QAT，下方是等价的普通按钮与最近一次命令回显。
    struct QatDemo {
        last: Option<SharedString>,
        focus_handle: FocusHandle,
    }

    impl QatDemo {
        /// 创建视图并聚焦自身。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            Self {
                last: None,
                focus_handle,
            }
        }

        /// 记录最近一次被派发的命令。
        fn record(&mut self, label: &str, cx: &mut Context<Self>) {
            self.last = Some(label.into());
            cx.notify();
        }

        fn on_save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("保存", cx);
        }

        fn on_undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("撤销", cx);
        }

        fn on_redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("重做", cx);
        }

        fn on_settings(&mut self, _: &Settings, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("设置", cx);
        }

        fn on_search(&mut self, _: &Search, _window: &mut Window, cx: &mut Context<Self>) {
            self.record("查找", cx);
        }
    }

    impl Render for QatDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let last = self
                .last
                .clone()
                .unwrap_or_else(|| SharedString::from("（尚未派发）"));

            v_flex()
                .id("qat-demo")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_save))
                .on_action(cx.listener(Self::on_undo))
                .on_action(cx.listener(Self::on_redo))
                .on_action(cx.listener(Self::on_settings))
                .on_action(cx.listener(Self::on_search))
                .size_full()
                .bg(rgb(0xf8fafc))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(rgb(0xf1f5f9))
                        .border_b_1()
                        .border_color(rgb(0xe2e8f0))
                        .child(
                            QuickAccessToolbar::new()
                                .item(QatItem::button("qat::Save").icon(IconName::Check))
                                .item(QatItem::button("qat::Undo").icon(IconName::Undo))
                                .item(QatItem::button("qat::Redo").icon(IconName::Redo))
                                .item(QatItem::separator())
                                .item(QatItem::button("qat::Search").label("查找"))
                                .item(QatItem::button("qat::Settings").icon(IconName::Settings)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!("最近派发：{last}")),
                        ),
                )
                .child(
                    v_flex()
                        .gap(px(10.0))
                        .p(px(16.0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(rgpui::FontWeight::BOLD)
                                .child("快速访问工具栏（QAT）demo"),
                        )
                        .child(div().text_sm().text_color(rgb(0x475569)).child(
                            "顶部一条 QAT 按命令 id 派发；下方按钮派发同一条命令，效果一致。",
                        ))
                        .child(div().text_sm().child(format!("最近派发：{last}")))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .items_center()
                                .child(
                                    div()
                                        .id("plain-save")
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .rounded_md()
                                        .bg(rgb(0xeef2ff))
                                        .cursor_pointer()
                                        .text_xs()
                                        .child("普通按钮：保存（qat::Save）")
                                        .on_click(|_, _window, cx| {
                                            let _ = cx.dispatch_command("qat::Save", None);
                                        }),
                                )
                                .child(
                                    div()
                                        .id("plain-settings")
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .rounded_md()
                                        .bg(rgb(0xeef2ff))
                                        .cursor_pointer()
                                        .text_xs()
                                        .child("普通按钮：设置（qat::Settings）")
                                        .on_click(|_, _window, cx| {
                                            let _ = cx.dispatch_command("qat::Settings", None);
                                        }),
                                ),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(620.0), px(340.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| QatDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| QatDemo::new(window, cx)).into()
    }
}

mod ribbon {
    //! Ribbon 功能区演示：数据驱动的选项卡/分组/大按钮/小按钮/切换/下拉，
    //! 命令按 id 绑定到 `CommandRegistry`，点击经 `App::dispatch_command` 派发到 `on_action`。

    use rgpui::ribbon::{
        Ribbon, RibbonComboBoxItem, RibbonContext, RibbonDropdownItem, RibbonGalleryItem,
        RibbonGroup, RibbonItem, RibbonState, RibbonTab,
    };
    use rgpui::{
        App, Bounds, CommandDescriptor, Context, FocusHandle, IconName, Render, Window,
        WindowBounds, WindowOptions, actions, div, prelude::*, px, register_command, rgb, size,
        v_flex,
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
                                    .item(
                                        RibbonComboBoxItem::new("demo::InsertTable").label("表格"),
                                    )
                                    .item(
                                        RibbonComboBoxItem::new("demo::InsertPicture")
                                            .label("图片"),
                                    )
                                    .item(
                                        RibbonComboBoxItem::new("demo::InsertChart").label("图表"),
                                    ),
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
                .on_action(
                    cx.listener(|this, _: &PasteSpecial, _, cx| this.record("选择性粘贴", cx)),
                )
                .on_action(cx.listener(|this, _: &Cut, _, cx| this.record("剪切", cx)))
                .on_action(cx.listener(|this, _: &Copy, _, cx| this.record("复制", cx)))
                .on_action(cx.listener(|this, _: &Bold, _, cx| {
                    let bold = !this.bold;
                    this.bold = bold;
                    this.record("加粗", cx);
                }))
                .on_action(cx.listener(|this, _: &StyleNormal, _, cx| this.record("常规样式", cx)))
                .on_action(cx.listener(|this, _: &StyleTitle, _, cx| this.record("标题样式", cx)))
                .on_action(
                    cx.listener(|this, _: &StyleEmphasis, _, cx| this.record("强调样式", cx)),
                )
                .on_action(cx.listener(|this, _: &InsertTable, _, cx| this.record("插入表格", cx)))
                .on_action(
                    cx.listener(|this, _: &InsertPicture, _, cx| this.record("插入图片", cx)),
                )
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

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| RibbonDemo::new(window, cx)).into()
    }
}

mod canvas_objects {
    //! 画布对象工具演示：选区 / 缩放手柄 / 旋转手柄 / 对齐 / 分布。
    //!
    //! 三个矩形对象放在可缩放平移的画布上：点击选中、拖动移动、拖手柄缩放、拖顶部旋转
    //! 手柄旋转；工具栏可左/水平居中/右对齐与水平分布。几何全部来自
    //! `rgpui::object_toolkit`，屏幕映射来自 `rgpui::viewport::Viewport`。

    use rgpui::object_toolkit::{
        AlignMode, ResizeHandle, SelectionHandles, align, distribute, hit_handle, resize_bounds,
        union_bounds,
    };
    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Axis, Bounds, Button, Context, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
        MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
        Styled, Window, WindowBounds, WindowOptions, div, h_flex, point, prelude::*, px, rgb, size,
        v_flex,
    };
    use rgpui_platform::application;

    /// 画布对象。
    struct CanvasObject {
        /// 边界（文档坐标）。
        rect: Bounds<Pixels>,
        /// 旋转角度（度，顺时针）。
        rotation: f32,
        /// 填充色。
        color: rgpui::Hsla,
        /// 标签。
        label: &'static str,
    }

    /// 拖拽模式。
    enum Drag {
        /// 未拖拽。
        None,
        /// 移动：记录起点文档坐标。
        Move(Point<Pixels>),
        /// 缩放：记录手柄。
        Resize(ResizeHandle),
        /// 旋转：记录中心、起始指针角度与起始角度。
        Rotate {
            /// 旋转中心（文档坐标）。
            center: Point<Pixels>,
            /// 起始指针相对中心的角度（弧度）。
            start_angle: f32,
            /// 起始旋转角度（度）。
            start_rotation: f32,
        },
    }

    /// 演示视图。
    struct CanvasObjectsDemo {
        /// 对象集合。
        objects: Vec<CanvasObject>,
        /// 当前选中对象下标。
        selected: Option<usize>,
        /// 视口。
        viewport: Viewport,
        /// 拖拽状态。
        drag: Drag,
        /// 焦点句柄。
        focus_handle: FocusHandle,
    }

    impl CanvasObjectsDemo {
        /// 创建演示视图。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let objects = vec![
                CanvasObject {
                    rect: Bounds::new(point(px(80.0), px(80.0)), size(px(220.0), px(140.0))),
                    rotation: 0.0,
                    color: rgb(0x60a5fa).into(),
                    label: "卡片 A",
                },
                CanvasObject {
                    rect: Bounds::new(point(px(360.0), px(120.0)), size(px(180.0), px(200.0))),
                    rotation: 0.0,
                    color: rgb(0x34d399).into(),
                    label: "卡片 B",
                },
                CanvasObject {
                    rect: Bounds::new(point(px(140.0), px(320.0)), size(px(260.0), px(120.0))),
                    rotation: 0.0,
                    color: rgb(0xfbbf24).into(),
                    label: "卡片 C",
                },
            ];
            Self {
                objects,
                selected: None,
                viewport: Viewport::new().with_offset(point(px(48.0), px(24.0))),
                drag: Drag::None,
                focus_handle: cx.focus_handle(),
            }
        }

        /// 选区（选中对象的边界，文档坐标）。
        fn selection(&self) -> Option<Bounds<Pixels>> {
            self.selected.map(|i| self.objects[i].rect)
        }

        /// 鼠标按下：命中手柄 → 缩放/旋转；命中对象 → 选中并移动；否则取消选择。
        fn on_mouse_down(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
            let doc = self.viewport.screen_to_document(screen);
            if let Some(selection) = self.selection()
                && let Some(handle) = hit_handle(selection, doc, px(7.0))
            {
                let index = self.selected.unwrap();
                if handle.is_rotate() {
                    let center = point(
                        selection.origin.x + selection.size.width / 2.0,
                        selection.origin.y + selection.size.height / 2.0,
                    );
                    let start_angle = (doc.y - center.y)
                        .as_f32()
                        .atan2((doc.x - center.x).as_f32());
                    self.drag = Drag::Rotate {
                        center,
                        start_angle,
                        start_rotation: self.objects[index].rotation,
                    };
                } else {
                    self.drag = Drag::Resize(handle);
                }
                cx.notify();
                return;
            }
            // 从上到下命中对象。
            let hit = (0..self.objects.len())
                .rev()
                .find(|i| self.objects[*i].rect.contains(&doc));
            self.selected = hit;
            self.drag = match hit {
                Some(_) => Drag::Move(doc),
                None => Drag::None,
            };
            cx.notify();
        }

        /// 鼠标移动：按拖拽模式更新。
        fn on_mouse_move(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
            let doc = self.viewport.screen_to_document(screen);
            let Some(index) = self.selected else {
                return;
            };
            match self.drag {
                Drag::Move(last) => {
                    let delta_x = doc.x - last.x;
                    let delta_y = doc.y - last.y;
                    let rect = &mut self.objects[index].rect;
                    rect.origin = point(rect.origin.x + delta_x, rect.origin.y + delta_y);
                    self.drag = Drag::Move(doc);
                    cx.notify();
                }
                Drag::Resize(handle) => {
                    let rect = self.objects[index].rect;
                    self.objects[index].rect = resize_bounds(rect, handle, doc, px(24.0));
                    cx.notify();
                }
                Drag::Rotate {
                    center,
                    start_angle,
                    start_rotation,
                } => {
                    let now = (doc.y - center.y)
                        .as_f32()
                        .atan2((doc.x - center.x).as_f32());
                    let delta = now - start_angle;
                    self.objects[index].rotation = start_rotation + delta.to_degrees();
                    cx.notify();
                }
                Drag::None => {}
            }
        }

        /// 鼠标释放：结束拖拽。
        fn on_mouse_up(&mut self, cx: &mut Context<Self>) {
            self.drag = Drag::None;
            cx.notify();
        }

        /// 对全部对象应用对齐。
        fn apply_align(&mut self, mode: AlignMode, cx: &mut Context<Self>) {
            let rects: Vec<Bounds<Pixels>> = self.objects.iter().map(|o| o.rect).collect();
            let aligned = align(&rects, mode);
            for (object, rect) in self.objects.iter_mut().zip(aligned) {
                object.rect = rect;
            }
            cx.notify();
        }

        /// 水平分布全部对象。
        fn apply_distribute(&mut self, cx: &mut Context<Self>) {
            let rects: Vec<Bounds<Pixels>> = self.objects.iter().map(|o| o.rect).collect();
            let distributed = distribute(&rects, Axis::Horizontal);
            for (object, rect) in self.objects.iter_mut().zip(distributed) {
                object.rect = rect;
            }
            cx.notify();
        }
    }

    impl Render for CanvasObjectsDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let viewport = self.viewport;
            let union = union_bounds(&self.objects.iter().map(|o| o.rect).collect::<Vec<_>>());

            // 对象层（后绘制的在上）。
            let objects: Vec<rgpui::AnyElement> = self
                .objects
                .iter()
                .enumerate()
                .map(|(index, object)| {
                    let origin = viewport.document_to_screen(object.rect.origin);
                    let size = viewport.document_size_to_screen(object.rect.size);
                    let selected = self.selected == Some(index);
                    div()
                        .absolute()
                        .left(origin.x)
                        .top(origin.y)
                        .w(size.width)
                        .h(size.height)
                        .rotate(object.rotation)
                        .rounded(px(8.0))
                        .border_2()
                        .border_color(if selected {
                            rgb(0x2563eb)
                        } else {
                            rgb(0x00000000)
                        })
                        .bg(object.color)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(rgb(0x0f172a))
                        .child(object.label)
                        .into_any_element()
                })
                .collect();

            let overlay = self.selection().map(|selection| {
                SelectionHandles::new(selection, viewport)
                    .rotate(true)
                    .into_any_element()
            });

            v_flex()
                .id("canvas-objects-demo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xf8fafc))
                .child(
                    h_flex()
                        .gap(px(6.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xcbd5e1))
                        .child(
                            div()
                                .mr(px(6.0))
                                .text_sm()
                                .font_weight(rgpui::FontWeight::SEMIBOLD)
                                .child("画布对象工具"),
                        )
                        .child(Button::new("obj-left").label("左对齐").on_click(
                            cx.listener(|this, _, _, cx| this.apply_align(AlignMode::Left, cx)),
                        ))
                        .child(
                            Button::new("obj-hcenter")
                                .label("水平居中")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.apply_align(AlignMode::CenterHorizontal, cx)
                                })),
                        )
                        .child(Button::new("obj-right").label("右对齐").on_click(
                            cx.listener(|this, _, _, cx| this.apply_align(AlignMode::Right, cx)),
                        ))
                        .child(
                            Button::new("obj-distribute")
                                .label("水平分布")
                                .on_click(cx.listener(|this, _, _, cx| this.apply_distribute(cx))),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!(
                                    "缩放 {:.0}% · 选中 {}",
                                    viewport.zoom() * 100.0,
                                    self.selected.map(|i| self.objects[i].label).unwrap_or("无")
                                )),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                this.on_mouse_down(event.position, cx);
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.on_mouse_move(event.position, cx);
                        }))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _, cx| this.on_mouse_up(cx)),
                        )
                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                            this.viewport.apply_wheel(event, px(50.0));
                            cx.notify();
                        }))
                        .children(objects)
                        .children(overlay)
                        .child(
                            div()
                                .absolute()
                                .left(px(12.0))
                                .bottom(px(8.0))
                                .text_xs()
                                .text_color(rgb(0x94a3b8))
                                .child(match union {
                                    Some(u) => format!(
                                        "对象外接框：{:.0} × {:.0}",
                                        u.size.width.as_f32(),
                                        u.size.height.as_f32()
                                    ),
                                    None => String::new(),
                                }),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(920.0), px(680.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| CanvasObjectsDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| CanvasObjectsDemo::new(window, cx)).into()
    }
}

mod ruler {
    //! 标尺与对齐参考线演示：顶部标尺刻度 + 拖拽方块互相吸附并显示参考线。
    //!
    //! 刻度由 `rgpui::guides::ruler_ticks` 计算，吸附由 `rgpui::guides::align` 计算。
    //! `Ctrl/⌘` + 滚轮缩放标尺步长（仅影响标尺演示）。

    use rgpui::guides::{Guide, GuideAxis, align, ruler_ticks};
    use rgpui::{
        App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
        Point, Render, ScrollWheelEvent, Window, WindowBounds, WindowOptions, bounds, canvas, div,
        fill, outline, point, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 标尺条高度。
    const RULER_HEIGHT: f32 = 28.0;
    /// 方块边长。
    const BOX: f32 = 80.0;
    /// 吸附阈值。
    const SNAP: f32 = 6.0;

    /// 演示视图。
    struct RulerDemo {
        zoom: f32,
        boxes: [Bounds<Pixels>; 2],
        dragging: Option<usize>,
        drag_offset: Point<Pixels>,
        guides: Vec<Guide>,
    }

    impl RulerDemo {
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self {
                zoom: 1.0,
                boxes: [
                    bounds(point(px(160.0), px(160.0)), size(px(BOX), px(BOX))),
                    bounds(point(px(360.0), px(280.0)), size(px(BOX), px(BOX))),
                ],
                dragging: None,
                drag_offset: point(px(0.0), px(0.0)),
                guides: Vec::new(),
            }
        }
    }

    impl Render for RulerDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let zoom = self.zoom;
            let boxes = self.boxes;
            let guides = self.guides.clone();

            v_flex().size_full().child(
                div()
                    .size_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            for (index, rect) in this.boxes.iter().enumerate() {
                                if rect.contains(&event.position) {
                                    this.dragging = Some(index);
                                    this.drag_offset = event.position - rect.origin;
                                    break;
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.dragging = None;
                            this.guides.clear();
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        let Some(index) = this.dragging else {
                            return;
                        };
                        let origin = event.position - this.drag_offset;
                        let mut rect = bounds(origin, size(px(BOX), px(BOX)));
                        let other = this.boxes[1 - index];
                        let result = align(rect, &[other], &[], &[], px(SNAP));
                        rect.origin += result.offset;
                        this.boxes[index] = rect;
                        this.guides = result.guides;
                        cx.notify();
                    }))
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        if event.modifiers.secondary() {
                            let dy = match event.delta {
                                rgpui::ScrollDelta::Lines(delta) => delta.y,
                                rgpui::ScrollDelta::Pixels(delta) => delta.y.as_f32() / 40.0,
                            };
                            this.zoom = (this.zoom * (1.0 + dy * 0.1)).clamp(0.25, 4.0);
                            cx.notify();
                        }
                    }))
                    .child(canvas(
                        |area, _window, _cx| area,
                        move |area, _state, window, _cx| {
                            window.paint_quad(fill(area, rgb(0xf8fafc)));

                            // 顶部标尺条。
                            let ruler =
                                bounds(area.origin, size(area.size.width, px(RULER_HEIGHT)));
                            window.paint_quad(fill(ruler, rgb(0xe2e8f0)));
                            window.paint_quad(outline(
                                ruler,
                                rgb(0xcbd5e1),
                                rgpui::BorderStyle::default(),
                            ));

                            // 刻度：可见文档区间 0..(宽度/zoom)。
                            let visible = area.size.width.as_f32() / zoom;
                            let ticks = ruler_ticks(0.0..visible, zoom, 60.0);
                            for (index, tick) in ticks.ticks.iter().enumerate() {
                                let x = area.origin.x + px(tick * zoom);
                                let major = index % 5 == 0;
                                let height = if major { 12.0 } else { 6.0 };
                                window.paint_quad(fill(
                                    bounds(
                                        point(x, area.origin.y + px(RULER_HEIGHT - height)),
                                        size(px(1.0), px(height)),
                                    ),
                                    rgb(0x64748b),
                                ));
                            }

                            // 方块。
                            for rect in boxes.iter() {
                                window.paint_quad(fill(*rect, rgb(0xdbeafe)));
                                window.paint_quad(outline(
                                    *rect,
                                    rgb(0x2563eb),
                                    rgpui::BorderStyle::default(),
                                ));
                            }

                            // 参考线。
                            for guide in guides.iter() {
                                let (line, color) = match guide.axis {
                                    GuideAxis::Vertical => (
                                        bounds(
                                            point(guide.position, area.origin.y),
                                            size(px(1.0), area.size.height),
                                        ),
                                        rgb(0xdc2626),
                                    ),
                                    GuideAxis::Horizontal => (
                                        bounds(
                                            point(area.origin.x, guide.position),
                                            size(area.size.width, px(1.0)),
                                        ),
                                        rgb(0xdc2626),
                                    ),
                                };
                                window.paint_quad(fill(line, color));
                            }
                        },
                    )),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(820.0), px(560.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| RulerDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| RulerDemo::new(window, cx)).into()
    }
}

mod viewport {
    //! 视口演示：可缩放/平移的 A4 文档页视图。
    //!
    //! 页面由 `canvas` 自绘，坐标全部经 `rgpui::viewport::Viewport` 在
    //! 文档坐标与屏幕坐标间换算；滚轮 `Ctrl/⌘` 以光标为中心缩放、否则平移，
    //! 左键拖拽平移。

    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Bounds, Button, Context, FocusHandle, MouseButton, MouseDownEvent, MouseMoveEvent,
        MouseUpEvent, Pixels, Point, Render, ScrollWheelEvent, Size, Window, WindowBounds,
        WindowOptions, bounds, canvas, div, fill, h_flex, outline, point, prelude::*, px, rgb,
        size, v_flex,
    };
    use rgpui_platform::application;

    /// A4 页面尺寸（96 DPI，像素）。
    fn page_size() -> Size<Pixels> {
        size(px(794.0), px(1123.0))
    }

    /// 页面边距。
    const MARGIN: f32 = 72.0;

    /// 演示视图。
    struct ViewportDemo {
        viewport: Viewport,
        focus_handle: FocusHandle,
        dragging: bool,
        last_pointer: Point<Pixels>,
        anchor: Point<Pixels>,
        view_size: Size<Pixels>,
    }

    impl ViewportDemo {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            Self {
                viewport: Viewport::new()
                    .with_zoom(0.6)
                    .with_offset(point(px(80.0), px(24.0))),
                focus_handle: cx.focus_handle(),
                dragging: false,
                last_pointer: point(px(0.0), px(0.0)),
                anchor: point(px(400.0), px(300.0)),
                view_size: size(px(800.0), px(600.0)),
            }
        }
    }

    impl Render for ViewportDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 记录视口可用尺寸，供「适应窗口」使用。
            self.view_size = window.bounds().size;

            // `Viewport` 为 `Copy`，复制一份供绘制闭包按值捕获。
            let viewport = self.viewport;
            let page = page_size();

            v_flex()
                .id("viewport-demo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xf1f5f9))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xcbd5e1))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(rgpui::FontWeight::SEMIBOLD)
                                .child("视口（Viewport）"),
                        )
                        .child(
                            Button::new("vp-zoom-out")
                                .label("缩小")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_out_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("vp-zoom-in")
                                .label("放大")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_in_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(Button::new("vp-100").label("100%").on_click(cx.listener(
                            |this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.set_zoom_at(1.0, anchor);
                                cx.notify();
                            },
                        )))
                        .child(
                            Button::new("vp-fit")
                                .label("适应窗口")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let view = this.view_size;
                                    this.viewport.fit(page_size(), view, px(32.0));
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!(
                                    "缩放 {:.0}% · 偏移 ({:.0}, {:.0})",
                                    viewport.zoom() * 100.0,
                                    viewport.offset().x.as_f32(),
                                    viewport.offset().y.as_f32()
                                )),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .relative()
                        .overflow_hidden()
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
                        .child(canvas(
                            |bounds, _window, _cx| bounds,
                            move |area, _state, window, _cx| {
                                let to_screen = |x: f32, y: f32| -> Point<Pixels> {
                                    viewport.document_to_screen(point(px(x), px(y)))
                                };
                                let doc_rect = |x: f32, y: f32, w: f32, h: f32| -> Bounds<Pixels> {
                                    bounds(
                                        to_screen(x, y),
                                        viewport.document_size_to_screen(size(px(w), px(h))),
                                    )
                                };

                                window.paint_quad(fill(area, rgb(0xe2e8f0)));

                                let paper =
                                    doc_rect(0.0, 0.0, page.width.as_f32(), page.height.as_f32());
                                window.paint_quad(fill(paper, rgb(0xffffff)));
                                window.paint_quad(outline(
                                    paper,
                                    rgb(0x94a3b8),
                                    rgpui::BorderStyle::default(),
                                ));

                                let content_width = page.width.as_f32() - MARGIN * 2.0;
                                window.paint_quad(fill(
                                    doc_rect(MARGIN, MARGIN, content_width, 40.0),
                                    rgb(0x1e293b),
                                ));

                                // 用等宽色条模拟正文段落。
                                let mut y = MARGIN + 80.0;
                                let mut line = 0usize;
                                while y < page.height.as_f32() - MARGIN {
                                    let width = if line % 5 == 4 {
                                        content_width * 0.6
                                    } else {
                                        content_width
                                    };
                                    window.paint_quad(fill(
                                        doc_rect(MARGIN, y, width, 10.0),
                                        rgb(0xcbd5e1),
                                    ));
                                    y += 24.0;
                                    line += 1;
                                }
                            },
                        )),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(900.0), px(680.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ViewportDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ViewportDemo::new(window, cx)).into()
    }
}

mod formula_bar {
    //! 公式栏演示：名称框 + fx + 公式输入，绑定一个 4×5 的迷你「单元格」模型。
    //! 名称框输入 `B3` 跳转、公式框回车写入当前单元格、fx 插入 `=SUM(`、Esc 取消。

    use rgpui::formula_bar::{FormulaBar, FormulaBarEvent, FormulaBarState};
    use rgpui::{
        App, Bounds, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled,
        Window, WindowBounds, WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
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

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| FormulaBarDemo::new(window, cx)).into()
    }
}

mod sheet_grid {
    //! 电子表格网格演示：`SheetGrid` + `FormulaBar` 绑定一个 8×6 的迷你表格。
    //!
    //! 鼠标拖拽选区、Shift 扩展、行列头点击整行/整列、方向键/PageUp/PageDown 移动、
    //! Ctrl+A 全选、双击或 Enter/F2 请求编辑、Delete 清空、右下角填充柄向下/向右填充。
    //! 选区变化会同步到公式栏的名称框与公式框，公式栏回车写回活动单元格。

    use rgpui::formula_bar::{FormulaBar, FormulaBarEvent, FormulaBarState};
    use rgpui::sheet_grid::{
        CellRef, SheetDelegate, SheetGrid, SheetGridEvent, SheetGridState, column_label,
    };
    use rgpui::{
        App, Bounds, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
        ParentElement, Pixels, Render, SharedString, Styled, Window, WindowBounds, WindowOptions,
        div, h_flex, prelude::*, px, rgb, size, v_flex,
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
        /// 用户拖拽调整过的列宽（列号 → 像素）。
        col_widths: std::collections::BTreeMap<usize, Pixels>,
        /// 用户拖拽调整过的行高（行号 → 像素）。
        row_heights: std::collections::BTreeMap<usize, Pixels>,
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
            Self {
                cells,
                col_widths: Default::default(),
                row_heights: Default::default(),
            }
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

        fn column_width(&self, col: usize, _cx: &App) -> Pixels {
            self.col_widths.get(&col).copied().unwrap_or(px(96.0))
        }

        fn row_height(&self, row: usize, _cx: &App) -> Pixels {
            self.row_heights.get(&row).copied().unwrap_or(px(24.0))
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
                        // 演示里没有单元格内输入框实体，传 None：网格进入编辑态但
                        // 单元格仍按普通格渲染（编辑经公式栏完成）。
                        this.grid
                            .update(cx, |grid, cx| grid.set_editing(Some(*cell), None, cx));
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
                    SheetGridEvent::ResetRowHeight { row } => {
                        // 双击行头分隔线：移除该行的自定义行高，回落默认值。
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().row_heights.remove(row);
                            cx.notify();
                        });
                        this.status = format!("第 {} 行行高已重置", row + 1);
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
                    SheetGridEvent::ColumnResized { col, width } => {
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().col_widths.insert(*col, *width);
                            cx.notify();
                        });
                        this.status =
                            format!("{} 列宽 {:.0}px", column_label(*col), width.as_f32());
                        cx.notify();
                    }
                    SheetGridEvent::RowResized { row, height } => {
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().row_heights.insert(*row, *height);
                            cx.notify();
                        });
                        this.status = format!("第 {} 行行高 {:.0}px", row + 1, height.as_f32());
                        cx.notify();
                    }
                    SheetGridEvent::AutoFitColumn { col } => {
                        // 演示用估算：按最长文本的字符数近似（实际应用可用文本系统精确测量）。
                        let sheet = this.grid.read(cx).delegate();
                        let max_chars = (0..sheet.rows_count(cx))
                            .map(|row| sheet.value(CellRef::new(row, *col)).chars().count())
                            .max()
                            .unwrap_or(4) as f32;
                        let width = px((max_chars * 9.0 + 16.0).clamp(40.0, 400.0));
                        this.grid.update(cx, |grid, cx| {
                            grid.delegate_mut().col_widths.insert(*col, width);
                            cx.notify();
                        });
                        this.status =
                            format!("{} 列自动宽度 {:.0}px", column_label(*col), width.as_f32());
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
                            grid.set_editing(None, None, cx);
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
                        this.grid
                            .update(cx, |grid, cx| grid.set_editing(None, None, cx));
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

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| SheetGridDemo::new(window, cx)).into()
    }
}

mod app_suite {
    //! 应用套件基座整合演示：一份命令表驱动 Ribbon / 按键提示 / 快速访问工具栏 /
    //! 应用菜单（Backstage）/ 分页视口（PageView + Viewport + 参考线）/ 迷你工具栏，
    //! 并用「编号标注 + 组件清单」把画面上每一块对应到哪个套件基座模块直接标出来。
    //!
    //! 与单功能示例（`ribbon` / `qat` / `backstage` / `page_view`）不同，本例把套件基座的
    //! 各块拼成一个可用的编辑器骨架：每个入口只写命令 id，点击经 `App::dispatch_command`
    //! 派发到同一批 `on_action`，改动同一份文档模型；状态栏回显
    //! `CommandRegistry::label_for` 取出的标签，用来证明「UI 文案」与「命令清单」同源。
    //!
    //! 组件对照：屏幕上的 ①…⑨ 编号就是 `Region` 枚举（本文件内唯一真源，同时驱动浮标、
    //! 侧边编号条与右侧清单），悬停任一编号即给对应区域描蓝边；Shift + 左键拖框选唤出
    //! `MiniToolbar`（它的入口与 QAT 一样都是 `QatItem` + 命令 id）。
    //!
    //! 运行：`cargo run -p showcase --bin app_suite`（依赖 `ribbon` + `viewport` + `document`
    //! feature，showcase 默认开启）。按 Alt/F10 显示按键提示，再按提示键即可纯键盘操作功能区；
    //! `Ctrl+Shift+E` 开关区域浮标、`Ctrl+Shift+I` 开关右侧组件清单、Esc 取消框选；
    //! 缩放与命令注册表同源：`Ctrl+↑`/`Ctrl+↓` 放大缩小、`Ctrl+0` 实际大小、`Ctrl+Shift+F` 适应窗口。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::backstage::{Backstage, BackstageState, BackstageTab};
    use rgpui::mini_toolbar::MiniToolbar;
    use rgpui::page_view::{
        PageArrangement, PageLabels, PageLayout, PageMargins, PageSetup, PageView,
    };
    use rgpui::qat::{QatItem, QuickAccessToolbar};
    use rgpui::ribbon::{
        Ribbon, RibbonContext, RibbonDropdownItem, RibbonGalleryItem, RibbonGroup, RibbonItem,
        RibbonState, RibbonTab,
    };
    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Bounds, ClickEvent, CommandDescriptor, CommandRegistry, Context, Entity, FocusHandle,
        IconName, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
        Point, Render, ScrollWheelEvent, SharedString, Size, Window, WindowBounds, WindowOptions,
        actions, div, h_flex, point, prelude::*, px, register_command, relative, rgb, size, v_flex,
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
            Recalc,
            ToggleCallouts,
            ToggleInspector
        ]
    );

    const NS: &str = "app_suite::";

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

    fn cmd_new() -> CommandDescriptor {
        command("New", "新建", "文件", Some("N"), Some("ctrl-n"))
    }
    fn cmd_open() -> CommandDescriptor {
        command("打开", "打开", "文件", Some("O"), Some("ctrl-o"))
    }
    fn cmd_save() -> CommandDescriptor {
        command("保存", "保存", "文件", Some("S"), Some("ctrl-s"))
    }
    fn cmd_print() -> CommandDescriptor {
        command("Print", "打印", "文件", Some("U"), Some("ctrl-p"))
    }
    fn cmd_paste() -> CommandDescriptor {
        command("Paste", "粘贴", "剪贴板", Some("V"), Some("ctrl-v"))
    }
    fn cmd_paste_special() -> CommandDescriptor {
        command("PasteSpecial", "选择性粘贴", "剪贴板", None, None)
    }
    fn cmd_cut() -> CommandDescriptor {
        command("Cut", "剪切", "剪贴板", Some("X"), Some("ctrl-x"))
    }
    fn cmd_copy() -> CommandDescriptor {
        command("复制", "复制", "剪贴板", Some("C"), Some("ctrl-c"))
    }
    fn cmd_bold() -> CommandDescriptor {
        command("Bold", "加粗", "字体", Some("B"), Some("ctrl-b"))
    }
    fn cmd_style_normal() -> CommandDescriptor {
        command("StyleNormal", "常规", "样式", None, None)
    }
    fn cmd_style_title() -> CommandDescriptor {
        command("StyleTitle", "标题", "样式", None, None)
    }
    fn cmd_style_emphasis() -> CommandDescriptor {
        command("StyleEmphasis", "强调", "样式", None, None)
    }
    fn cmd_insert_table() -> CommandDescriptor {
        command("InsertTable", "表格", "插入", Some("T"), None)
    }
    fn cmd_insert_picture() -> CommandDescriptor {
        command("InsertPicture", "图片", "插入", Some("I"), None)
    }
    fn cmd_new_page() -> CommandDescriptor {
        command("NewPage", "分页", "插入", Some("G"), None)
    }
    fn cmd_zoom_in() -> CommandDescriptor {
        command("ZoomIn", "放大", "视图", Some("Z"), Some("ctrl-up"))
    }
    fn cmd_zoom_out() -> CommandDescriptor {
        command("ZoomOut", "缩小", "视图", Some("Q"), Some("ctrl-down"))
    }
    fn cmd_zoom_actual() -> CommandDescriptor {
        command("ZoomActual", "100%", "视图", Some("A"), Some("ctrl-0"))
    }
    fn cmd_zoom_fit() -> CommandDescriptor {
        command(
            "ZoomFit",
            "适应窗口",
            "视图",
            Some("F"),
            Some("ctrl-shift-f"),
        )
    }
    fn cmd_layout_single() -> CommandDescriptor {
        command("LayoutSingle", "单页", "视图", None, None)
    }
    fn cmd_layout_two_up() -> CommandDescriptor {
        command("LayoutTwoUp", "对开", "视图", None, None)
    }
    fn cmd_layout_cover() -> CommandDescriptor {
        command("LayoutCover", "封面对开", "视图", None, None)
    }
    fn cmd_toggle_margins() -> CommandDescriptor {
        command("ToggleMargins", "窄边距", "视图", Some("M"), None)
    }
    fn cmd_toggle_guides() -> CommandDescriptor {
        command("ToggleGuides", "内容参考线", "视图", Some("D"), None)
    }
    fn cmd_collapse_ribbon() -> CommandDescriptor {
        command("CollapseRibbon", "折叠功能区", "视图", Some("R"), None)
    }
    fn cmd_recalc() -> CommandDescriptor {
        command("Recalc", "重新计算", "数据", Some("L"), Some("f9"))
    }
    fn cmd_toggle_callouts() -> CommandDescriptor {
        command(
            "ToggleCallouts",
            "组件标注",
            "视图",
            Some("E"),
            Some("ctrl-shift-e"),
        )
    }
    fn cmd_toggle_inspector() -> CommandDescriptor {
        command(
            "ToggleInspector",
            "组件清单",
            "视图",
            Some("W"),
            Some("ctrl-shift-i"),
        )
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
    register_command!(cmd_toggle_callouts);
    register_command!(cmd_toggle_inspector);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum DocStyle {
        /// 常规正文。
        Normal,
        /// 标题（更大更深）。
        Title,
        /// 强调（琥珀色）。
        Emphasis,
    }

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

    struct Page {
        /// 本页内容块。
        blocks: Vec<Block>,
    }

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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Region {
        /// 快速访问工具栏。
        Qat,
        /// 应用菜单（Backstage）。
        Backstage,
        /// 功能区。
        Ribbon,
        /// 按键提示。
        KeyTips,
        /// 分页画布。
        PageView,
        /// 视口（缩放 / 平移）。
        Viewport,
        /// 页面内容参考线（`PageView` 的虚线框开关）。
        Guides,
        /// 迷你工具栏。
        MiniToolbar,
        /// 命令注册表。
        Commands,
    }

    impl Region {
        /// 中文模块名（浮标与清单标题）。
        fn title(self) -> &'static str {
            match self {
                Region::Qat => "快速访问工具栏",
                Region::Backstage => "应用菜单",
                Region::Ribbon => "功能区",
                Region::KeyTips => "按键提示",
                Region::PageView => "分页画布",
                Region::Viewport => "视口",
                Region::Guides => "内容参考线",
                Region::MiniToolbar => "迷你工具栏",
                Region::Commands => "命令注册表",
            }
        }

        /// 模块名（类型 / 函数名）。
        fn module(self) -> &'static str {
            match self {
                Region::Qat => "QuickAccessToolbar",
                Region::Backstage => "Backstage",
                Region::Ribbon => "Ribbon",
                Region::KeyTips => "KeyTips",
                Region::PageView => "PageView",
                Region::Viewport => "Viewport",
                Region::Guides => "PageView::content_guide",
                Region::MiniToolbar => "MiniToolbar",
                Region::Commands => "CommandRegistry",
            }
        }

        /// 源文件与所属 feature。
        fn source(self) -> &'static str {
            match self {
                Region::Qat => "qat.rs · ribbon",
                Region::Backstage => "backstage.rs · ribbon",
                Region::Ribbon => "ribbon.rs · ribbon",
                Region::KeyTips => "keytips.rs · ribbon",
                Region::PageView => "page_view.rs · document",
                Region::Viewport => "viewport.rs · viewport",
                Region::Guides => "page_view.rs · document",
                Region::MiniToolbar => "mini_toolbar.rs · ribbon",
                Region::Commands => "commands.rs · 核心",
            }
        }

        /// 在本示例里的触发方式。
        fn trigger(self) -> &'static str {
            match self {
                Region::Qat => "顶栏图标条，每项只写命令 id",
                Region::Backstage => "点顶栏「文件」开合整窗菜单",
                Region::Ribbon => "四个选项卡 + 「数据」上下文选项卡",
                Region::KeyTips => "按 Alt 或 F10 显示按键提示，再按字母",
                Region::PageView => "画布上的纸页、页眉页脚与页间距",
                Region::Viewport => "滚轮缩放、左键拖拽平移、视图 › 适应窗口",
                Region::Guides => "视图 › 内容参考线（虚线框即内容区）",
                Region::MiniToolbar => "Shift + 左键拖框选，命令条就近浮出",
                Region::Commands => "状态栏回显 label_for，条数即注册表规模",
            }
        }

        /// 带圈编号（按画面顺序：顶栏 → 功能区 → 画布 → 状态栏，与 [`REGIONS`] 一致）。
        fn badge(self) -> &'static str {
            match self {
                Region::Backstage => "①",
                Region::Qat => "②",
                Region::Ribbon => "③",
                Region::KeyTips => "④",
                Region::PageView => "⑤",
                Region::Viewport => "⑥",
                Region::Guides => "⑦",
                Region::MiniToolbar => "⑧",
                Region::Commands => "⑨",
            }
        }
    }

    const REGIONS: [Region; 9] = [
        Region::Backstage,
        Region::Qat,
        Region::Ribbon,
        Region::KeyTips,
        Region::PageView,
        Region::Viewport,
        Region::Guides,
        Region::MiniToolbar,
        Region::Commands,
    ];

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
        /// 是否显示组件标注（编号浮标 + 区域描边）。
        callouts: bool,
        /// 是否显示右侧组件清单面板。
        inspector: bool,
        /// 当前被高亮的组件（清单悬停时给出）。
        highlighted: Option<Region>,
        /// 框选区间（窗口坐标，Shift + 左键拖拽）。
        selection: Option<(Point<Pixels>, Point<Pixels>)>,
        /// 是否正在框选。
        marquee: bool,
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
                callouts: true,
                inspector: true,
                highlighted: None,
                selection: None,
                marquee: false,
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

        /// 组件标注浮标：编号 + 中文模块名，高亮时补出源文件与 feature。
        ///
        /// `slot` 用来区分同一组件在不同位置的浮标（顶栏 / 功能区 / 标注条 / 清单），
        /// 避免元素 id 冲突；悬停浮标即把对应组件设为高亮。
        fn tag(&self, region: Region, slot: &str, entity: Entity<Self>) -> rgpui::AnyElement {
            let active = self.highlighted == Some(region);
            h_flex()
                .id(format!("tag-{slot}-{}", region.module()))
                .flex_shrink_0()
                .items_center()
                .gap(px(4.0))
                .px(px(6.0))
                .py(px(2.0))
                .rounded_full()
                .cursor_pointer()
                .text_xs()
                .bg(if active { rgb(0x1d4ed8) } else { rgb(0x475569) })
                .text_color(rgb(0xf8fafc))
                .on_hover(move |hovered, _window, app| {
                    entity.update(app, |this, cx| {
                        this.highlighted = if *hovered { Some(region) } else { None };
                        cx.notify();
                    });
                })
                .child(region.badge())
                .child(region.title())
                .when(active, |this| this.child(format!("· {}", region.source())))
                .into_any_element()
        }

        /// 编号方块：只占一个方块宽，挂在各区域的标注轨道上（悬停即高亮该区域）。
        fn chip(&self, region: Region, entity: Entity<Self>) -> rgpui::AnyElement {
            let active = self.highlighted == Some(region);
            div()
                .id(format!("chip-{}", region.module()))
                .flex_shrink_0()
                .w(px(22.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .bg(if active { rgb(0x2563eb) } else { rgb(0x334155) })
                .text_color(rgb(0xf8fafc))
                .on_hover(move |hovered, _window, app| {
                    entity.update(app, |this, cx| {
                        this.highlighted = if *hovered { Some(region) } else { None };
                        cx.notify();
                    });
                })
                .child(region.badge())
                .into_any_element()
        }

        /// 竖向标注轨道：把若干区域的编号方块贴在对应区域旁边。
        fn rail(&self, regions: &[Region], entity: Entity<Self>) -> rgpui::AnyElement {
            v_flex()
                .w(px(30.0))
                .flex_shrink_0()
                .items_center()
                .gap(px(3.0))
                .py(px(3.0))
                .bg(rgb(0x0f172a))
                .children(
                    regions
                        .iter()
                        .map(|region| self.chip(*region, entity.clone())),
                )
                .into_any_element()
        }

        /// 标注说明行：悬停时给出该组件的源文件、feature 与触发方式，
        /// 平时给出整套标注操作口径（浮标 / 清单 / 迷你工具栏）。
        fn hint_line(&self) -> rgpui::AnyElement {
            let text = match self.highlighted {
                Some(region) => format!(
                    "{} {} · {} —— {}",
                    region.badge(),
                    region.title(),
                    region.source(),
                    region.trigger()
                ),
                None => "编号方块 = 组件标注：悬停方块或右侧清单行即高亮对应区域；\
                 Ctrl+Shift+E 开关浮标，Ctrl+Shift+I 开关清单；Shift + 左键拖框选唤出迷你工具栏。"
                    .to_string(),
            };
            h_flex()
                .w_full()
                .flex_shrink_0()
                .px(px(10.0))
                .py(px(3.0))
                .bg(rgb(0x0f172a))
                .text_xs()
                .text_color(rgb(0xcbd5e1))
                .child(text)
                .into_any_element()
        }

        /// 右侧组件清单：编号 / 模块 / 源文件与 feature / 触发方式，悬停行高亮对应区域。
        fn inspector_panel(&self, entity: Entity<Self>) -> rgpui::AnyElement {
            v_flex()
                .id("suite-inspector")
                .w(px(288.0))
                .flex_shrink_0()
                .border_l_1()
                .border_color(rgb(0xcbd5e1))
                .bg(rgb(0xf8fafc))
                .px(px(10.0))
                .py(px(8.0))
                .gap(px(5.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .text_sm()
                        .font_weight(rgpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0x0f172a))
                        .child("本演示用到的套件基座组件"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child("悬停任意一行 → 界面上对应区域点亮描边；Ctrl+Shift+I 开关本面板。"),
                )
                .children(REGIONS.iter().map(|region| {
                    let region = *region;
                    let active = self.highlighted == Some(region);
                    let entity = entity.clone();
                    v_flex()
                        .id(format!("insp-{}", region.module()))
                        .w_full()
                        .flex_shrink_0()
                        .gap(px(1.0))
                        .px(px(6.0))
                        .py(px(4.0))
                        .rounded_md()
                        .cursor_pointer()
                        .border_1()
                        .border_color(if active { rgb(0x2563eb) } else { rgb(0xe2e8f0) })
                        .bg(if active { rgb(0xdbeafe) } else { rgb(0xffffff) })
                        .on_hover(move |hovered, _window, app| {
                            entity.update(app, |this, cx| {
                                this.highlighted = if *hovered { Some(region) } else { None };
                                cx.notify();
                            });
                        })
                        .child(
                            h_flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .text_xs()
                                        .bg(if active { rgb(0x2563eb) } else { rgb(0x64748b) })
                                        .text_color(rgb(0xffffff))
                                        .child(region.badge()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(rgpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(0x0f172a))
                                        .child(region.title()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x475569))
                                        .child(region.module()),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(region.source()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x334155))
                                .child(region.trigger()),
                        )
                }))
                .child(div().text_xs().text_color(rgb(0x64748b)).child(
                    "未在本文档区演示：公式栏 / 电子表格网格 → showcase --bin spreadsheet；\
                         控制通道 / MCP → showcase --bin automation。",
                ))
                .into_any_element()
        }

        /// 迷你工具栏的落点：框选终点右下偏移，避免压住选区。
        fn toolbar_at(selection: (Point<Pixels>, Point<Pixels>)) -> Point<Pixels> {
            point(selection.1.x + px(6.0), selection.1.y + px(20.0))
        }

        /// 功能区选项卡：所有条目只写命令 id，标签由命令描述符提供。
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
                    )
                    .group(
                        RibbonGroup::new("演示标注")
                            .item(
                                RibbonItem::toggle("app_suite::ToggleCallouts")
                                    .label("区域浮标")
                                    .keytip("E")
                                    .icon(IconName::Eye)
                                    .checked(self.callouts),
                            )
                            .item(
                                RibbonItem::toggle("app_suite::ToggleInspector")
                                    .label("组件清单")
                                    .keytip("W")
                                    .icon(IconName::PanelRight)
                                    .checked(self.inspector),
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
                                        if let Err(error) =
                                            cx.dispatch_command("app_suite::New", None)
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
            self.record("打开", cx);
        }

        fn on_save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
            self.dirty = false;
            self.record("保存", cx);
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

        fn on_paste_special(
            &mut self,
            _: &PasteSpecial,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
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
            self.record("复制", cx);
        }

        fn on_bold(&mut self, _: &Bold, _window: &mut Window, cx: &mut Context<Self>) {
            self.bold = !self.bold;
            self.touch();
            self.record("Bold", cx);
        }

        fn on_style_normal(
            &mut self,
            _: &StyleNormal,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
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

        fn on_insert_table(
            &mut self,
            _: &InsertTable,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
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

        fn on_layout_single(
            &mut self,
            _: &LayoutSingle,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.arrangement = PageArrangement::Single;
            self.record("LayoutSingle", cx);
        }

        fn on_layout_two_up(
            &mut self,
            _: &LayoutTwoUp,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.arrangement = PageArrangement::TwoUp;
            self.record("LayoutTwoUp", cx);
        }

        fn on_layout_cover(
            &mut self,
            _: &LayoutCover,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
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

        fn on_toggle_guides(
            &mut self,
            _: &ToggleGuides,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
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

        fn on_toggle_callouts(
            &mut self,
            _: &ToggleCallouts,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.callouts = !self.callouts;
            if !self.callouts {
                self.highlighted = None;
            }
            self.record("ToggleCallouts", cx);
        }

        fn on_toggle_inspector(
            &mut self,
            _: &ToggleInspector,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.inspector = !self.inspector;
            if !self.inspector {
                self.highlighted = None;
            }
            self.record("ToggleInspector", cx);
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
            // 高亮判定：清单 / 方块悬停中落在哪块区域，就给对应区域描蓝边。
            let header_hot = matches!(
                self.highlighted,
                Some(Region::Backstage) | Some(Region::Qat)
            );
            let ribbon_hot = matches!(
                self.highlighted,
                Some(Region::Ribbon) | Some(Region::KeyTips)
            );
            let canvas_hot = matches!(
                self.highlighted,
                Some(Region::PageView)
                    | Some(Region::Viewport)
                    | Some(Region::Guides)
                    | Some(Region::MiniToolbar)
            );
            let status_hot = self.highlighted == Some(Region::Commands);
            let selection = self.selection;
            // 悬停高亮走实体句柄：浮标与清单行都通过它改写 `highlighted`。
            let entity = cx.entity();

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
                .on_action(cx.listener(Self::on_toggle_callouts))
                .on_action(cx.listener(Self::on_toggle_inspector))
                .on_key_down(
                    cx.listener(|this, event: &rgpui::KeyDownEvent, _window, cx| {
                        // Esc 取消框选（迷你工具栏随之收起）。
                        if event.keystroke.key == "escape" && this.selection.take().is_some() {
                            cx.notify();
                        }
                    }),
                )
                // 顶栏：① 文件入口（Backstage）+ ② 快速访问工具栏 + 保存状态。
                .child(
                    h_flex()
                        .w_full()
                        .flex_shrink_0()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(rgb(0x1e293b))
                        .border_b_2()
                        .border_color(if header_hot {
                            rgb(0x38bdf8)
                        } else {
                            rgb(0x1e293b)
                        })
                        .when(self.callouts, |this| {
                            this.child(self.tag(Region::Backstage, "head", entity.clone()))
                        })
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
                        .when(self.callouts, |this| {
                            this.child(self.tag(Region::Qat, "head", entity.clone()))
                        })
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
                .child(
                    // 功能区（③）与按键提示（④）：左侧编号轨道 + 高亮描边。
                    h_flex()
                        .w_full()
                        .flex_shrink_0()
                        .items_stretch()
                        .when(self.callouts, |this| {
                            this.child(
                                self.rail(&[Region::Ribbon, Region::KeyTips], entity.clone()),
                            )
                        })
                        .child(
                            div()
                                .flex_grow_1()
                                .min_w(px(0.0))
                                .border_2()
                                .border_color(if ribbon_hot {
                                    rgb(0x2563eb)
                                } else {
                                    rgb(0xe2e8f0)
                                })
                                .child(Ribbon::new(&self.ribbon, tabs)),
                        ),
                )
                // 内容区：标注轨道（⑤⑥⑦⑧）+ 分页画布 + 视口 + 右侧组件清单。
                // 滚轮 Ctrl/⌘ 缩放、左键拖拽平移（视口）；Shift + 左键拖框选（迷你工具栏）。
                .child(
                    h_flex()
                        .w_full()
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .items_stretch()
                        .when(self.callouts, |this| {
                            this.child(self.rail(
                                &[
                                    Region::PageView,
                                    Region::Viewport,
                                    Region::Guides,
                                    Region::MiniToolbar,
                                ],
                                entity.clone(),
                            ))
                        })
                        .child(
                            div()
                                .relative()
                                .flex_grow_1()
                                .min_w(px(0.0))
                                .min_h(px(0.0))
                                .overflow_hidden()
                                .border_2()
                                .border_color(if canvas_hot {
                                    rgb(0x2563eb)
                                } else {
                                    rgb(0xe2e8f0)
                                })
                                .bg(rgb(0xf1f5f9))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                        if event.modifiers.shift {
                                            this.marquee = true;
                                            this.selection = Some((event.position, event.position));
                                        } else {
                                            this.dragging = true;
                                            this.last_pointer = event.position;
                                        }
                                        cx.notify();
                                    }),
                                )
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                        this.dragging = false;
                                        this.marquee = false;
                                        cx.notify();
                                    }),
                                )
                                .on_mouse_move(cx.listener(
                                    |this, event: &MouseMoveEvent, _, cx| {
                                        this.anchor = event.position;
                                        if this.marquee {
                                            if let Some((start, _)) = this.selection {
                                                this.selection = Some((start, event.position));
                                            }
                                        } else if this.dragging {
                                            let delta = event.position - this.last_pointer;
                                            this.last_pointer = event.position;
                                            this.viewport.pan_by(delta);
                                        }
                                        cx.notify();
                                    },
                                ))
                                .on_scroll_wheel(cx.listener(
                                    |this, event: &ScrollWheelEvent, _, cx| {
                                        this.viewport.apply_wheel(event, px(50.0));
                                        cx.notify();
                                    },
                                ))
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
                        .when(self.inspector, |this| {
                            this.child(self.inspector_panel(entity.clone()))
                        }),
                )
                .when(self.callouts, |this| this.child(self.hint_line()))
                // 状态栏：⑨ 命令注册表 + 最近命令、命令清单规模、待重算表格、视口读数。
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
                        .bg(if status_hot {
                            rgb(0xdbeafe)
                        } else {
                            rgb(0xf8fafc)
                        })
                        .text_xs()
                        .text_color(rgb(0x475569))
                        .child(format!("最近命令：{last}"))
                        .child(format!(
                            "命令清单 {} 条 · 待重算 {stale} 张 · 排布 {arrangement} · {margins}",
                            CommandRegistry::global().len()
                        ))
                        .when(self.callouts, |this| {
                            this.child(div().ml_auto().child(self.tag(
                                Region::Commands,
                                "status",
                                entity.clone(),
                            )))
                        })
                        .child(div().ml_auto().child(format!("缩放 {:.0}%", zoom * 100.0))),
                )
                // 框选矩形与迷你工具栏：鼠标事件给的是窗口坐标，所以挂在根容器上。
                .children(selection.map(|(start, end)| {
                    div()
                        .absolute()
                        .left(start.x.min(end.x))
                        .top(start.y.min(end.y))
                        .w((start.x - end.x).abs())
                        .h((start.y - end.y).abs())
                        .border_1()
                        .border_color(rgb(0x2563eb))
                        .bg(rgb(0x60a5fa).opacity(0.25))
                }))
                .children(selection.map(|selection| {
                    MiniToolbar::new()
                        .at(Self::toolbar_at(selection))
                        .item(QatItem::button("app_suite::Bold").label("B"))
                        .item(QatItem::button("app_suite::StyleTitle").label("标题"))
                        .item(QatItem::separator())
                        .item(QatItem::button("app_suite::Copy").icon(IconName::Copy))
                }))
                .child(Backstage::new(&self.backstage, backstage_tabs).sidebar_width(px(150.0)))
        }
    }

    pub fn run_example() {
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
                KeyBinding::new("ctrl-up", ZoomIn, None),
                KeyBinding::new("ctrl-down", ZoomOut, None),
                KeyBinding::new("ctrl-0", ZoomActual, None),
                KeyBinding::new("ctrl-shift-f", ZoomFit, None),
                KeyBinding::new("f9", Recalc, None),
                KeyBinding::new("ctrl-shift-e", ToggleCallouts, None),
                KeyBinding::new("ctrl-shift-i", ToggleInspector, None),
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| AppSuite::new(window, cx)).into()
    }
}

mod automation {
    //! 控制通道 / MCP 演示：外部进程经本机 TCP 调命令，AI 客户端经 stdio 调 MCP 工具。
    //!
    //! 启动后控制台打印监听地址与密钥（同时显示在窗口里）：
    //!
    //! ```text
    //! RGPUI_CONTROL_CHANNEL=127.0.0.1:52341
    //! RGPUI_CONTROL_KEY=0123456789abcdef0123456789abcdef
    //! ```
    //!
    //! PowerShell 里手动试一条（把地址/密钥换成上面打印的值）：
    //!
    //! ```powershell
    //! $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 52341)
    //! $stream = $client.GetStream()
    //! $writer = [System.IO.StreamWriter]::new($stream)
    //! $writer.WriteLine('{"jsonrpc":"2.0","id":1,"key":"<密钥>","method":"commands.list"}')
    //! $writer.Flush()
    //! [System.IO.StreamReader]::new($stream).ReadLine()
    //! ```
    //!
    //! 带 `--mcp` 启动时额外在 stdin/stdout 上跑 MCP 服务端（握手信息改打到 stderr，
    //! 保证 stdout 只有 JSON-RPC）：
    //!
    //! ```text
    //! cargo run -p showcase --bin automation -- --mcp
    //! -> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}
    //! <- {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18",...}}
    //! -> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"automation_demo__Increment","arguments":{}}}
    //! ```
    //!
    //! 窗口里的计数器会随 `command.dispatch`（TCP）或 `tools/call`（MCP）实时变化。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::control_channel::{ControlChannel, ControlChannelOptions};
    use rgpui::mcp::McpServer;
    use rgpui::{
        App, Bounds, CommandDescriptor, Context, FocusHandle, Render, Window, WindowBounds,
        WindowOptions, actions, div, h_flex, prelude::*, px, register_command, size, v_flex,
    };
    use rgpui_platform::application;

    actions!(automation_demo, [Increment, Decrement, Reset]);

    fn cmd_increment() -> CommandDescriptor {
        CommandDescriptor::new("automation_demo::Increment", "增加")
            .with_category("演示")
            .with_description("把计数器加一")
            .with_keystroke("ctrl-i")
    }

    fn cmd_decrement() -> CommandDescriptor {
        CommandDescriptor::new("automation_demo::Decrement", "减少")
            .with_category("演示")
            .with_description("把计数器减一")
    }

    fn cmd_reset() -> CommandDescriptor {
        CommandDescriptor::new("automation_demo::Reset", "归零")
            .with_category("演示")
            .with_description("把计数器清零")
    }

    register_command!(cmd_increment);
    register_command!(cmd_decrement);
    register_command!(cmd_reset);

    struct AutomationDemo {
        count: i32,
        focus_handle: FocusHandle,
        /// 控制通道句柄：随视图存活，视图释放即关闭监听。
        _channel: ControlChannel,
        /// MCP 服务端句柄（仅在 `--mcp` 时存在）。
        _mcp: Option<McpServer>,
        addr: String,
        key: String,
        mcp_enabled: bool,
    }

    impl AutomationDemo {
        fn new(
            window: &mut Window,
            cx: &mut Context<Self>,
            channel: ControlChannel,
            mcp: Option<McpServer>,
        ) -> Self {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            let addr = channel.addr().to_string();
            let key = channel.key().to_string();
            let mcp_enabled = mcp.is_some();
            Self {
                count: 0,
                focus_handle,
                _channel: channel,
                _mcp: mcp,
                addr,
                key,
                mcp_enabled,
            }
        }

        fn on_increment(&mut self, _: &Increment, _window: &mut Window, cx: &mut Context<Self>) {
            self.count += 1;
            cx.notify();
        }

        fn on_decrement(&mut self, _: &Decrement, _window: &mut Window, cx: &mut Context<Self>) {
            self.count -= 1;
            cx.notify();
        }

        fn on_reset(&mut self, _: &Reset, _window: &mut Window, cx: &mut Context<Self>) {
            self.count = 0;
            cx.notify();
        }
    }

    impl Render for AutomationDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let request = format!(
                r#"{{"jsonrpc":"2.0","id":1,"key":"{}","method":"command.dispatch","params":{{"id":"automation_demo::Increment"}}}}"#,
                self.key
            );

            v_flex()
                .id("automation-demo")
                .track_focus(&self.focus_handle)
                .on_action(_cx.listener(Self::on_increment))
                .on_action(_cx.listener(Self::on_decrement))
                .on_action(_cx.listener(Self::on_reset))
                .size_full()
                .gap(px(10.0))
                .p(px(16.0))
                .bg(rgpui::rgb(0xf8fafc))
                .child(
                    div()
                        .text_lg()
                        .font_weight(rgpui::FontWeight::BOLD)
                        .child("控制通道 / MCP 演示"),
                )
                .child(div().text_sm().child(format!("监听地址：{}", self.addr)))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgpui::rgb(0x64748b))
                        .child(format!("访问密钥：{}", self.key)),
                )
                .child(div().text_xs().text_color(rgpui::rgb(0x64748b)).child(
                    if self.mcp_enabled {
                        "MCP：已启用（stdin/stdout，握手信息在 stderr）"
                    } else {
                        "MCP：未启用（加 --mcp 开启）"
                    },
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgpui::rgb(0x334155))
                        .child(format!("计数器：{}", self.count)),
                )
                .child(
                    div()
                        .rounded_md()
                        .bg(rgpui::rgb(0x0f172a))
                        .p(px(8.0))
                        .text_xs()
                        .text_color(rgpui::rgb(0xe2e8f0))
                        .child(request),
                )
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(
                            div()
                                .id("automation-increment")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("增加（automation_demo::Increment）")
                                .on_click(|_, _window, cx| {
                                    let _ = cx.dispatch_command("automation_demo::Increment", None);
                                }),
                        )
                        .child(
                            div()
                                .id("automation-reset")
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .child("归零")
                                .on_click(|_, _window, cx| {
                                    let _ = cx.dispatch_command("automation_demo::Reset", None);
                                }),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);

            let channel = cx
                .start_control_channel(ControlChannelOptions::default())
                .expect("控制通道启动失败");
            let addr = channel.addr();
            let key = channel.key().to_string();
            let mcp_enabled = std::env::args().any(|arg| arg == "--mcp");
            let mcp = if mcp_enabled {
                Some(McpServer::start_stdio(cx).expect("MCP 服务端启动失败"))
            } else {
                None
            };

            // `--mcp` 时 stdout 只能出现 JSON-RPC，握手信息改走 stderr。
            if mcp_enabled {
                eprintln!("RGPUI_CONTROL_CHANNEL={addr}");
                eprintln!("RGPUI_CONTROL_KEY={key}");
            } else {
                println!("RGPUI_CONTROL_CHANNEL={addr}");
                println!("RGPUI_CONTROL_KEY={key}");
            }

            let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| AutomationDemo::new(window, cx, channel, mcp)),
            )
            .unwrap();
            cx.activate(true);
        });
    }
}

/// Gallery 根视图：左侧竖行 tab + 右侧示例内容。
struct GalleryApp {
    titles: Vec<&'static str>,
    /// 当前示例的介绍与用法。
    active: usize,
    views: Vec<AnyView>,
}

impl GalleryApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let titles: Vec<&'static str> = vec![
            "backstage",
            "command_registry",
            "mini_toolbar",
            "qat",
            "ribbon",
            "canvas_objects",
            "ruler",
            "viewport",
            "formula_bar",
            "sheet_grid",
            "app_suite",
        ];
        let builds: &[fn(&mut Window, &mut App) -> AnyView] = &[
            backstage::build,
            command_registry::build,
            mini_toolbar::build,
            qat::build,
            ribbon::build,
            canvas_objects::build,
            ruler::build,
            viewport::build,
            formula_bar::build,
            sheet_grid::build,
            app_suite::build,
        ];
        let views = builds.iter().map(|build| build(window, cx)).collect();
        Self {
            titles,
            active: 0,
            views,
        }
    }
}

impl Render for GalleryApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let theme = cx.theme();
        div()
            .id("gallery-app")
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .id("gallery-tabs")
                    .h_full()
                    .w(px(180.0))
                    .flex_shrink_0()
                    .flex_col()
                    .overflow_y_scroll()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(self.titles.iter().enumerate().map(|(ix, title)| {
                        let this = this.clone();
                        Button::new(("gallery-tab", ix))
                            .label(*title)
                            .w_full()
                            .when(ix == self.active, |b| b.primary())
                            .when(ix != self.active, |b| b.ghost())
                            .on_click(move |_, _, cx| {
                                this.update(cx, |gallery, cx| {
                                    gallery.active = ix;
                                    cx.notify();
                                });
                            })
                    })),
            )
            .child(
                div()
                    .id("gallery-content")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.views[self.active].clone()),
            )
    }
}

fn run() {
    // 传 -- <demo> 时保留原有独立窗口行为；不带参数时打开 gallery 导航界面。
    if let Some(name) = std::env::args().nth(1) {
        match name.as_str() {
            "backstage" => backstage::run_example(),
            "command_registry" => command_registry::run_example(),
            "mini_toolbar" => mini_toolbar::run_example(),
            "qat" => qat::run_example(),
            "ribbon" => ribbon::run_example(),
            "canvas_objects" => canvas_objects::run_example(),
            "ruler" => ruler::run_example(),
            "viewport" => viewport::run_example(),
            "formula_bar" => formula_bar::run_example(),
            "sheet_grid" => sheet_grid::run_example(),
            "app_suite" => app_suite::run_example(),
            "automation" => automation::run_example(),
            _ => eprintln!("未知 demo：{name}"),
        }
        return;
    }
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| GalleryApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}
