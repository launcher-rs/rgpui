//! rgpui v1.1.0 功能展示示例
//!
//! 运行：`cargo run -p v1_1_showcase`

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::components::status_bar::LspStatus;
use rgpui::prelude::*;
use rgpui::tabs::tab_drag::{TabDragDrop, TabDragState, TabItem};
use rgpui::{
    AnyElement, Button, ButtonVariants as _, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, Window, WindowOptions, div, h_flex, px, rgb, size, v_flex,
};
use rgpui_platform::application;

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
struct AppConfig {
    theme: String,
    font_size: u32,
    auto_save: bool,
    language: String,
}

impl AppConfig {
    fn demo() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14,
            auto_save: true,
            language: "zh-CN".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavSection {
    I18n,
    Config,
    Theme,
    BlockRender,
    VirtualScroll,
    SourceMap,
    TabDrag,
    StatusBar,
    FpsHud,
    Chat,
}

/// 自定义状态栏状态。
#[derive(Debug, Clone)]
struct CustomStatusBarState {
    line: usize,
    column: usize,
    language: String,
    encoding: String,
    lsp_status: LspStatus,
    lsp_server_name: String,
    error_count: usize,
    warning_count: usize,
    git_branch: String,
}

impl Default for CustomStatusBarState {
    fn default() -> Self {
        Self {
            line: 42,
            column: 15,
            language: "Rust".to_string(),
            encoding: "UTF-8".to_string(),
            lsp_status: LspStatus::Connected,
            lsp_server_name: "rust-analyzer".to_string(),
            error_count: 2,
            warning_count: 5,
            git_branch: "main".to_string(),
        }
    }
}

struct ShowcaseApp {
    current_section: NavSection,
    i18n_locale: String,
    i18n_manager: I18nManager,
    config: AppConfig,
    config_store_path: Option<std::path::PathBuf>,
    theme_mode: ThemeMode,
    block_renderer: BlockRenderer,
    markdown_input: String,
    virtual_scroll_items: Vec<String>,
    source_input: String,
    chat_messages: Vec<Message>,
    tab_drag_state: Entity<TabDragState>,
    status_bar: CustomStatusBarState,
}

impl ShowcaseApp {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut i18n = I18nManager::new("zh-CN");
        let mut en = std::collections::HashMap::new();
        en.insert("greeting".to_string(), "Hello, {name}!".to_string());
        en.insert("welcome".to_string(), "Welcome to rgpui!".to_string());
        i18n.load_translations_map("en", en);
        let mut zh = std::collections::HashMap::new();
        zh.insert("greeting".to_string(), "你好，{name}！".to_string());
        zh.insert("welcome".to_string(), "欢迎使用 rgpui！".to_string());
        i18n.load_translations_map("zh-CN", zh);

        let virtual_scroll_items: Vec<String> = (0..100).map(|i| format!("项目 #{}", i)).collect();
        let chat_messages = vec![
            Message::text("你好！欢迎使用 rgpui Chat UI"),
            Message::text("这是一个消息列表组件"),
            Message::code_block("rust", "let msg = Message::text(\"hello\");"),
        ];

        let tab_drag_state = cx.new(|_| TabDragState {
            enabled: true,
            tabs: vec![
                TabItem {
                    title: "main.rs".to_string(),
                    id: "t1".to_string(),
                    closable: true,
                },
                TabItem {
                    title: "lib.rs".to_string(),
                    id: "t2".to_string(),
                    closable: true,
                },
                TabItem {
                    title: "mod.rs".to_string(),
                    id: "t3".to_string(),
                    closable: false,
                },
                TabItem {
                    title: "utils.rs".to_string(),
                    id: "t4".to_string(),
                    closable: true,
                },
            ],
            ..Default::default()
        });

        Self {
            current_section: NavSection::I18n,
            i18n_locale: "zh-CN".to_string(),
            i18n_manager: i18n,
            config: AppConfig::demo(),
            config_store_path: None,
            theme_mode: ThemeMode::Light,
            block_renderer: BlockRenderer::new(),
            markdown_input:
                "# 标题\n\n这是 **粗体** 和 *斜体*\n\n## 子标题\n\n- 列表项 1\n- 列表项 2"
                    .to_string(),
            virtual_scroll_items,
            source_input: "fn main() {\n    println!(\"Hello\");\n}".to_string(),
            chat_messages,
            tab_drag_state,
            status_bar: CustomStatusBarState::default(),
        }
    }
}

impl Render for ShowcaseApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nav_items = [
            (NavSection::I18n, "I18n 国际化"),
            (NavSection::Config, "ConfigStore 配置"),
            (NavSection::Theme, "ThemeWatcher 主题"),
            (NavSection::BlockRender, "BlockRender 块渲染"),
            (NavSection::VirtualScroll, "VirtualScroll 虚拟滚动"),
            (NavSection::SourceMap, "SourceMap 源码映射"),
            (NavSection::TabDrag, "TabDrag 标签拖拽"),
            (NavSection::StatusBar, "StatusBar 状态栏"),
            (NavSection::FpsHud, "FpsHud 性能监控"),
            (NavSection::Chat, "ChatUI 聊天"),
        ];

        let current = self.current_section;
        let i18n_locale = self.i18n_locale.clone();
        let greeting = self.i18n_manager.t("greeting", &[("name", "开发者")]);
        let welcome = self.i18n_manager.t("welcome", &[]);
        let config = self.config.clone();
        let theme_mode = self.theme_mode.clone();
        let markdown_input = self.markdown_input.clone();
        let source_input = self.source_input.clone();
        let virtual_scroll_items = self.virtual_scroll_items.clone();
        let chat_messages = self.chat_messages.clone();
        let block_renderer = &self.block_renderer;
        let tab_drag_state = self.tab_drag_state.clone();
        let sb = self.status_bar.clone();

        let nav = v_flex()
            .id("nav")
            .w(px(200.0))
            .h_full()
            .bg(rgb(0xf8f9fa))
            .border_r(px(1.0))
            .border_color(rgb(0xe9ecef))
            .p(px(12.0))
            .gap(px(4.0))
            .child(
                div()
                    .text_lg()
                    .font_bold()
                    .mb(px(8.0))
                    .child("v1.1.0 功能展示"),
            )
            .children(nav_items.into_iter().map(|(section, label)| {
                let is_active = current == section;
                let label = label.to_string();
                div()
                    .id(format!("nav-{:?}", section))
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .text_sm()
                    .when(is_active, |el| {
                        el.bg(rgb(0x0078d4)).text_color(rgb(0xffffff))
                    })
                    .when(!is_active, |el| el.hover(|el| el.bg(rgb(0xe9ecef))))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.current_section = section;
                        cx.notify();
                    }))
            }));

        let content = match current {
            NavSection::I18n => v_flex().id("i18n-section").gap(px(16.0)).p(px(24.0))
                .child(section_title("I18n 国际化支持"))
                .child(section_desc("动态语言切换，支持变量插值和复数规则"))
                .child(h_flex().gap(px(8.0))
                    .child(Button::new("i18n-zh").label("中文").when(i18n_locale == "zh-CN", |b| b.primary()).when(i18n_locale != "zh-CN", |b| b.ghost())
                        .on_click(cx.listener(|this, _, _, cx| { this.i18n_locale = "zh-CN".to_string(); this.i18n_manager.set_locale("zh-CN"); cx.notify(); })))
                    .child(Button::new("i18n-en").label("English").when(i18n_locale == "en", |b| b.primary()).when(i18n_locale != "en", |b| b.ghost())
                        .on_click(cx.listener(|this, _, _, cx| { this.i18n_locale = "en".to_string(); this.i18n_manager.set_locale("en"); cx.notify(); }))))
                .child(card("翻译结果", &[format!("greeting: {}", greeting), format!("welcome: {}", welcome)]))
                .child(code_block("let mut i18n = I18nManager::new(\"zh-CN\");\ni18n.set_locale(\"en\");\nprintln!(\"{}\", i18n.t(\"greeting\", &[(\"name\", \"开发者\")]));")),

            NavSection::Config => {
                let cd = config.clone();
                v_flex().id("config-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("ConfigStore 配置持久化"))
                    .child(section_desc("JSON 配置文件的保存、加载和监听"))
                    .child(v_flex().gap(px(8.0))
                        .child(h_flex().gap(px(8.0)).child("主题: ").child(div().px(px(8.0)).py(px(4.0)).rounded(px(4.0)).bg(rgb(0xe9ecef)).child(cd.theme)))
                        .child(h_flex().gap(px(8.0)).child("字号: ").child(div().px(px(8.0)).py(px(4.0)).rounded(px(4.0)).bg(rgb(0xe9ecef)).child(format!("{}", cd.font_size))))
                        .child(h_flex().gap(px(8.0)).child("自动保存: ").child(div().px(px(8.0)).py(px(4.0)).rounded(px(4.0)).bg(rgb(0xe9ecef)).child(if cd.auto_save { "开启" } else { "关闭" }))))
                    .child(h_flex().gap(px(8.0))
                        .child(Button::new("config-save").label("保存配置").primary().on_click(cx.listener(|this, _, _, cx| {
                            let temp = tempfile::tempdir().unwrap(); let path = temp.path().join("config.json");
                            let mut store = ConfigStore::with_path(path.clone()); store.save(&this.config).unwrap();
                            this.config_store_path = Some(path); cx.notify(); })))
                        .child(Button::new("config-load").label("加载配置").ghost().on_click(cx.listener(|this, _, _, cx| {
                            if let Some(path) = &this.config_store_path { let mut store = ConfigStore::with_path(path.clone());
                            if let Ok(config) = store.load::<AppConfig>() { this.config = config; cx.notify(); } } }))))
                    .child(code_block("#[derive(Serialize, Deserialize)]\nstruct AppConfig { theme: String, font_size: u32 }\n\nlet mut store = ConfigStore::with_path(\"config.json\");\nstore.save(&config)?;\nlet loaded: AppConfig = store.load()?;"))
            }

            NavSection::Theme => v_flex().id("theme-section").gap(px(16.0)).p(px(24.0))
                .child(section_title("ThemeWatcher 主题热重载"))
                .child(section_desc("监听系统主题变化，支持手动切换"))
                .child(h_flex().gap(px(8.0))
                    .child(Button::new("theme-light").label("亮色").when(theme_mode == ThemeMode::Light, |b| b.primary()).when(theme_mode != ThemeMode::Light, |b| b.ghost())
                        .on_click(cx.listener(|this, _, _, cx| { this.theme_mode = ThemeMode::Light; cx.notify(); })))
                    .child(Button::new("theme-dark").label("暗色").when(theme_mode == ThemeMode::Dark, |b| b.primary()).when(theme_mode != ThemeMode::Dark, |b| b.ghost())
                        .on_click(cx.listener(|this, _, _, cx| { this.theme_mode = ThemeMode::Dark; cx.notify(); })))
                    .child(Button::new("theme-system").label("跟随系统").when(theme_mode == ThemeMode::System, |b| b.primary()).when(theme_mode != ThemeMode::System, |b| b.ghost())
                        .on_click(cx.listener(|this, _, _, cx| { this.theme_mode = ThemeMode::System; cx.notify(); }))))
                .child(v_flex().gap(px(4.0)).child(div().text_sm().font_medium().child("当前主题:"))
                    .child(div().px(px(8.0)).py(px(4.0)).rounded(px(4.0)).bg(rgb(0xe9ecef)).child(format!("{:?}", theme_mode))))
                .child(code_block("let mut watcher = ThemeWatcher::new();\nwatcher.set_theme(ThemeMode::Dark);")),

            NavSection::BlockRender => {
                let blocks = block_renderer.parse_markdown(&markdown_input);
                v_flex().id("block-render-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("BlockRender 块级渲染"))
                    .child(section_desc("Markdown 文本的块级元素解析与渲染"))
                    .child(v_flex().gap(px(4.0)).child(div().text_sm().font_medium().child("解析结果:"))
                        .children(blocks.into_iter().map(|block| render_block_element(&block))))
                    .child(code_block("let renderer = BlockRenderer::new();\nlet blocks = renderer.parse_markdown(\"# 标题\\n\\n**粗体** 和 *斜体*\");"))
            }

            NavSection::VirtualScroll => {
                let item_count = virtual_scroll_items.len();
                v_flex().id("virtual-scroll-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("VirtualScroll 虚拟滚动"))
                    .child(section_desc("仅渲染可视区域内的列表项，支持大量数据"))
                    .child(div().text_sm().font_medium().child(format!("数据量: {} 项", item_count)))
                    .child(div().id("virtual-scroll-container").h(px(300.0)).rounded(px(6.0)).border(px(1.0)).border_color(rgb(0xe9ecef)).overflow_scroll()
                        .child(div().id("virtual-scroll-content").w_full()
                            .children((0..item_count).map(|i| {
                                div().id(format!("vs-item-{}", i)).w_full().h(px(32.0)).flex().items_center().px(px(12.0)).text_sm()
                                    .border_b(px(1.0)).border_color(rgb(0xf0f0f0)).when(i % 2 == 0, |el| el.bg(rgb(0xf8f9fa)))
                                    .child(format!("{} - 项目 #{}", i, i))
                            }))))
                    .child(card("配置信息", &["direction: Vertical".to_string(), "buffer_size: 5".to_string(), "estimated_item_height: 32px".to_string()]))
                    .child(code_block("let config = VirtualScrollConfig { direction: Vertical, buffer_size: 5, estimated_item_height: 32.0 };\nVirtualScroll::new(state).config(config)"))
            }

            NavSection::SourceMap => {
                let source_map = SourceMap::new("example.rs", &source_input);
                let line_count = source_map.line_count();
                let lines: Vec<String> = (1..=line_count.min(5)).filter_map(|i| source_map.get_line(i).map(|l| format!("{}: {}", i, l))).collect();
                v_flex().id("source-map-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("SourceMap 源码映射"))
                    .child(section_desc("源码行号/列号定位与双向映射"))
                    .child(v_flex().gap(px(4.0)).child(div().text_sm().font_medium().child("源码:"))
                        .child(div().px(px(8.0)).py(px(6.0)).rounded(px(6.0)).bg(rgb(0x282c34)).text_sm().text_color(rgb(0xabb2bf)).child(source_input)))
                    .child(card("解析结果", &[format!("总行数: {}", line_count), format!("搜索 'println': {} 处", source_map.search("println").len())]))
                    .child(v_flex().gap(px(4.0)).child(div().text_sm().font_medium().child("行内容:"))
                        .children(lines.into_iter().map(|l| div().px(px(8.0)).py(px(2.0)).text_sm().child(l))))
                    .child(code_block("let source_map = SourceMap::new(\"main.rs\", source);\nprintln!(\"行数: {}\", source_map.line_count());"))
            }

            NavSection::TabDrag => {
                let tabs = tab_drag_state.read(cx).tabs.clone();
                let tab_drag = TabDragDrop::new(tab_drag_state.clone())
                    .on_reorder(|_tabs, _window, _cx| {
                        println!("Tab 重新排序完成");
                    });
                v_flex().id("tab-drag-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("TabDrag 标签拖拽排序"))
                    .child(section_desc("拖拽 Tab 进行排序，支持 on_drag/drag_over/on_drop 事件"))
                    .child(tab_drag)
                    .child(h_flex().gap(px(8.0))
                        .child(Button::new("move-left").label("左移").ghost().on_click(cx.listener(|this, _, _, cx| {
                            let state = this.tab_drag_state.read(cx);
                            if let Some(first) = state.tabs.iter().position(|t| t.closable)
                                && first > 0 { let (from, to) = (first, first - 1); let _ = state;
                                    this.tab_drag_state.update(cx, |s, _| s.move_tab(from, to)); cx.notify(); } })))
                        .child(Button::new("move-right").label("右移").ghost().on_click(cx.listener(|this, _, _, cx| {
                            let state = this.tab_drag_state.read(cx);
                            if let Some(first) = state.tabs.iter().position(|t| t.closable)
                                && first < state.tabs.len() - 1 { let (from, to) = (first, first + 1); let _ = state;
                                    this.tab_drag_state.update(cx, |s, _| s.move_tab(from, to)); cx.notify(); } })))
                        .child(Button::new("reset-tabs").label("重置").primary().on_click(cx.listener(|this, _, _, cx| {
                            this.tab_drag_state.update(cx, |s, _| {
                                s.tabs = vec![
                                    TabItem { title: "main.rs".to_string(), id: "t1".to_string(), closable: true },
                                    TabItem { title: "lib.rs".to_string(), id: "t2".to_string(), closable: true },
                                    TabItem { title: "mod.rs".to_string(), id: "t3".to_string(), closable: false },
                                    TabItem { title: "utils.rs".to_string(), id: "t4".to_string(), closable: true },
                                ]; }); cx.notify(); }))))
                    .child(card("拖拽状态", &[format!("Tab 数量: {}", tabs.len()), "拖拽 Tab 可进行排序，蓝色线条指示目标位置".to_string()]))
                    .child(code_block("let drag_state = cx.new(|_| TabDragState::default());\nTabDragDrop::new(drag_state)\n    .on_reorder(|tabs, window, cx| { /* 处理排序 */ })"))
            }

            NavSection::StatusBar => {
                let sb = sb.clone();
                v_flex().id("status-bar-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("StatusBar 状态栏"))
                    .child(section_desc("显示编辑器状态信息，支持动态修改"))
                    .child(
                        // 交互按钮区
                        h_flex().gap(px(8.0)).flex_wrap()
                            .child(Button::new("sb-line-up").label("行号+1").ghost().on_click(cx.listener(|this, _, _, cx| { this.status_bar.line += 1; cx.notify(); })))
                            .child(Button::new("sb-line-down").label("行号-1").ghost().on_click(cx.listener(|this, _, _, cx| { this.status_bar.line = this.status_bar.line.saturating_sub(1).max(1); cx.notify(); })))
                            .child(Button::new("sb-col-up").label("列号+5").ghost().on_click(cx.listener(|this, _, _, cx| { this.status_bar.column += 5; cx.notify(); })))
                            .child(Button::new("sb-lang").label("切换语言").ghost().on_click(cx.listener(|this, _, _, cx| {
                                this.status_bar.language = if this.status_bar.language == "Rust" { "TypeScript".to_string() } else { "Rust".to_string() }; cx.notify(); })))
                            .child(Button::new("sb-lsp").label("切换LSP").ghost().on_click(cx.listener(|this, _, _, cx| {
                                this.status_bar.lsp_status = match this.status_bar.lsp_status {
                                    LspStatus::Connected => LspStatus::Disconnected,
                                    LspStatus::Disconnected => LspStatus::Initializing,
                                    LspStatus::Initializing => LspStatus::Connected,
                                    LspStatus::Error => LspStatus::Connected,
                                }; cx.notify(); })))
                            .child(Button::new("sb-error").label("错误+1").ghost().on_click(cx.listener(|this, _, _, cx| { this.status_bar.error_count += 1; cx.notify(); })))
                            .child(Button::new("sb-warn").label("警告+1").ghost().on_click(cx.listener(|this, _, _, cx| { this.status_bar.warning_count += 1; cx.notify(); })))
                            .child(Button::new("sb-reset").label("重置").primary().on_click(cx.listener(|this, _, _, cx| { this.status_bar = CustomStatusBarState::default(); cx.notify(); }))),
                    )
                    .child(card("当前状态", &[
                        format!("行: {}, 列: {}", sb.line, sb.column),
                        format!("语言: {}", sb.language),
                        format!("编码: {}", sb.encoding),
                        format!("LSP: {:?} ({})", sb.lsp_status, sb.lsp_server_name),
                        format!("错误: {}, 警告: {}", sb.error_count, sb.warning_count),
                        format!("Git: {}", sb.git_branch),
                    ]))
                    .child(code_block("let status = cx.new(|_| StatusBarState {\n    line: 42, column: 15,\n    language: \"Rust\".into(),\n    lsp_status: LspStatus::Connected,\n    ..Default::default()\n});\nStatusBar::new(status)"))
            }

            NavSection::FpsHud => {
                let fps = 60.0; let frame_time = 16.67;
                let color = if fps >= 55.0 { rgb(0x28a745) } else if fps >= 30.0 { rgb(0xffc107) } else { rgb(0xdc3545) };
                v_flex().id("fps-hud-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("FpsHud 性能监控"))
                    .child(section_desc("实时 FPS/CPU/GPU 监控显示"))
                    .child(h_flex().gap(px(24.0))
                        .child(v_flex().gap(px(4.0)).child(div().text_xs().text_color(rgb(0x666)).child("FPS")).child(div().text_3xl().font_bold().text_color(color).child(format!("{:.0}", fps))))
                        .child(v_flex().gap(px(4.0)).child(div().text_xs().text_color(rgb(0x666)).child("帧时间")).child(div().text_3xl().font_bold().child(format!("{:.2}ms", frame_time))))
                        .child(v_flex().gap(px(4.0)).child(div().text_xs().text_color(rgb(0x666)).child("状态")).child(div().text_3xl().font_bold().text_color(color).child("流畅"))))
                    .child(card("颜色规则", &["FPS >= 55: 绿色 (流畅)".to_string(), "FPS >= 30: 黄色 (卡顿)".to_string(), "FPS < 30: 红色 (严重卡顿)".to_string()]))
                    .child(code_block("let state = cx.new(FpsHudState::new);\nFpsHud::new(state).position(Point::new(px(10.0), px(10.0))).show_cpu(true)"))
            }

            NavSection::Chat => {
                v_flex().id("chat-section").gap(px(16.0)).p(px(24.0))
                    .child(section_title("ChatUI 聊天组件"))
                    .child(section_desc("支持消息分组、多类型的聊天界面"))
                    .child(v_flex().id("chat-messages").gap(px(8.0)).h(px(250.0)).rounded(px(6.0)).border(px(1.0)).border_color(rgb(0xe9ecef)).p(px(12.0)).overflow_scroll()
                        .children(chat_messages.iter().map(|msg| {
                            let (bg, tc) = match &msg.content { MessageType::Text(_) => (rgb(0xf8f9fa), rgb(0x333333)), MessageType::CodeBlock { .. } => (rgb(0x282c34), rgb(0xabb2bf)) };
                            let ct = match &msg.content { MessageType::Text(s) => s.clone(), MessageType::CodeBlock { code, .. } => code.clone() };
                            div().id(msg.id.clone()).px(px(10.0)).py(px(6.0)).rounded(px(6.0)).bg(bg).text_sm().text_color(tc).child(ct)
                        })))
                    .child(card("消息类型", &["MessageType::Text(String) — 纯文本".to_string(), "MessageType::CodeBlock { language, code } — 代码块".to_string()]))
                    .child(code_block("let msg = Message::text(\"你好\");\nlet code = Message::code_block(\"rust\", \"fn main() {}\");"))
            }
        };

        // 自定义状态栏（匹配白色主题）
        let status_bar = custom_status_bar(&sb);

        // 主布局：导航 + 内容 + 底部状态栏
        v_flex()
            .id("showcase-app")
            .size_full()
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(nav)
                    .child(content.flex_1()),
            )
            .child(status_bar)
    }
}

// ============================================================================
// 自定义状态栏（白色主题）
// ============================================================================

fn custom_status_bar(sb: &CustomStatusBarState) -> impl IntoElement {
    let lsp_color = match sb.lsp_status {
        LspStatus::Connected => rgb(0x28a745),
        LspStatus::Initializing => rgb(0xffc107),
        LspStatus::Disconnected => rgb(0x6c757d),
        LspStatus::Error => rgb(0xdc3545),
    };
    let lsp_text = match sb.lsp_status {
        LspStatus::Connected => format!("{} ✓", sb.lsp_server_name),
        LspStatus::Initializing => "LSP...".to_string(),
        LspStatus::Disconnected => "No LSP".to_string(),
        LspStatus::Error => "LSP Error".to_string(),
    };

    h_flex()
        .id("custom-status-bar")
        .w_full()
        .h(px(28.0))
        .items_center()
        .justify_between()
        .px(px(12.0))
        .bg(rgb(0xf0f0f0))
        .border_t(px(1.0))
        .border_color(rgb(0xe0e0e0))
        .child(
            h_flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(lsp_color))
                        .child(div().text_xs().child(lsp_text)),
                )
                .children(if sb.error_count > 0 || sb.warning_count > 0 {
                    Some(
                        h_flex()
                            .items_center()
                            .gap(px(8.0))
                            .children(if sb.error_count > 0 {
                                Some(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xdc3545))
                                        .child(format!("{} errors", sb.error_count)),
                                )
                            } else {
                                None
                            })
                            .children(if sb.warning_count > 0 {
                                Some(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0xffc107))
                                        .child(format!("{} warnings", sb.warning_count)),
                                )
                            } else {
                                None
                            }),
                    )
                } else {
                    None
                }),
        )
        .child(
            h_flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(div().text_xs().text_color(rgb(0x666)).child(" "))
                        .child(div().text_xs().child(sb.git_branch.clone())),
                )
                .child(
                    div()
                        .text_xs()
                        .child(format!("Ln {}, Col {}", sb.line, sb.column)),
                )
                .child(div().text_xs().child(sb.language.clone()))
                .child(div().text_xs().child(sb.encoding.clone())),
        )
}

// ============================================================================
// 辅助组件
// ============================================================================

fn render_inline_text(text: &str) -> Vec<AnyElement> {
    let mut elements = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(start) = remaining.find("**") {
            if start > 0 {
                elements.push(
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child(remaining[..start].to_string())
                        .into_any(),
                );
            }
            remaining = &remaining[start + 2..];
            if let Some(end) = remaining.find("**") {
                elements.push(
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .font_bold()
                        .child(remaining[..end].to_string())
                        .into_any(),
                );
                remaining = &remaining[end + 2..];
            } else {
                elements.push(div().text_sm().child("**".to_string()).into_any());
                break;
            }
        } else if let Some(start) = remaining.find('*') {
            if start > 0 {
                elements.push(
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child(remaining[..start].to_string())
                        .into_any(),
                );
            }
            remaining = &remaining[start + 1..];
            if let Some(end) = remaining.find('*') {
                elements.push(
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .italic()
                        .child(remaining[..end].to_string())
                        .into_any(),
                );
                remaining = &remaining[end + 1..];
            } else {
                elements.push(div().text_sm().child("*".to_string()).into_any());
                break;
            }
        } else {
            elements.push(
                div()
                    .text_sm()
                    .text_color(rgb(0x333333))
                    .child(remaining.to_string())
                    .into_any(),
            );
            break;
        }
    }
    elements
}

fn render_block_element(block: &BlockElement) -> AnyElement {
    match &block.block_type {
        BlockType::Heading(level) => {
            let d = div()
                .id(format!("h-{}", block.content))
                .mb(px(4.0))
                .text_color(rgb(0x1a1a1a));
            let d = match level {
                1 => d.text_2xl().font_bold(),
                2 => d.text_xl().font_semibold(),
                3 => d.text_lg().font_medium(),
                _ => d.text_base(),
            };
            d.child(block.content.clone()).into_any_element()
        }
        BlockType::Paragraph => {
            let els = render_inline_text(&block.content);
            div()
                .id(format!(
                    "p-{}",
                    block.content.chars().take(8).collect::<String>()
                ))
                .mb(px(8.0))
                .flex()
                .gap(px(0.0))
                .children(els)
                .into_any_element()
        }
        BlockType::CodeBlock => {
            let lang = block
                .attributes
                .get("language")
                .map(|s| s.as_str())
                .unwrap_or("");
            div()
                .id("code")
                .mb(px(8.0))
                .rounded(px(6.0))
                .bg(rgb(0x282c34))
                .p(px(12.0))
                .child(
                    v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x666))
                                .child(format!("语言: {}", lang)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0xabb2bf))
                                .child(block.content.clone()),
                        ),
                )
                .into_any_element()
        }
        BlockType::Blockquote => div()
            .id("quote")
            .mb(px(8.0))
            .pl(px(12.0))
            .border_l(px(3.0))
            .border_color(rgb(0x0078d4))
            .text_sm()
            .text_color(rgb(0x666))
            .italic()
            .child(block.content.clone())
            .into_any_element(),
        BlockType::HorizontalRule => div()
            .id("hr")
            .my(px(12.0))
            .h(px(1.0))
            .bg(rgb(0xe9ecef))
            .into_any_element(),
        _ => div()
            .text_sm()
            .child(block.content.clone())
            .into_any_element(),
    }
}

fn section_title(text: &str) -> impl IntoElement {
    div().text_xl().font_bold().child(text.to_string())
}
fn section_desc(text: &str) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(rgb(0x666666))
        .child(text.to_string())
}
fn card(title: &str, items: &[String]) -> impl IntoElement {
    v_flex()
        .gap(px(8.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(rgb(0xffffff))
        .border(px(1.0))
        .border_color(rgb(0xe9ecef))
        .child(div().text_sm().font_medium().child(title.to_string()))
        .children(items.iter().map(|item| {
            div()
                .text_sm()
                .text_color(rgb(0x333333))
                .child(item.clone())
        }))
}
fn code_block(code: &str) -> impl IntoElement {
    div()
        .rounded(px(8.0))
        .bg(rgb(0x282c34))
        .p(px(12.0))
        .text_sm()
        .text_color(rgb(0xabb2bf))
        .child(code.to_string())
}

fn main() {
    application().run(|cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(rgpui::WindowBounds::Windowed(rgpui::Bounds::new(
                    rgpui::Point::default(),
                    size(px(1000.0), px(700.0)),
                ))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ShowcaseApp::new(window, cx)),
        )
        .unwrap();
    });
}
