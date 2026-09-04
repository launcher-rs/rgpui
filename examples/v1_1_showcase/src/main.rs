//! rgpui v1.1.0 功能展示示例
//!
//! 通过交互式界面展示 v1.1.0 中所有新增功能的使用方法。
//!
//! 运行：
//! ```text
//! cargo run -p v1_1_showcase
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::prelude::*;
use rgpui::{
    Button, ButtonVariants as _, Context, InteractiveElement, IntoElement, ParentElement, Render,
    Window, WindowOptions, div, h_flex, px, rgb, size, v_flex,
};
use rgpui_platform::application;

// ============================================================================
// 数据类型
// ============================================================================

/// 应用配置（用于 ConfigStore 示例）。
#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
struct AppConfig {
    theme: String,
    font_size: u32,
    auto_save: bool,
    language: String,
}

impl AppConfig {
    /// 创建带有真实默认值的配置。
    fn demo() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14,
            auto_save: true,
            language: "zh-CN".to_string(),
        }
    }
}

/// 导航项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavSection {
    I18n,
    Config,
    Theme,
    BlockRender,
    VirtualScroll,
    SourceMap,
    TabDrag,
    FpsHud,
    Chat,
}

// ============================================================================
// 主视图
// ============================================================================

/// 应用根视图。
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
}

impl ShowcaseApp {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        // 初始化 I18n
        let mut i18n = I18nManager::new("zh-CN");
        let mut en = std::collections::HashMap::new();
        en.insert("greeting".to_string(), "Hello, {name}!".to_string());
        en.insert("welcome".to_string(), "Welcome to rgpui!".to_string());
        i18n.load_translations_map("en", en);

        let mut zh = std::collections::HashMap::new();
        zh.insert("greeting".to_string(), "你好，{name}！".to_string());
        zh.insert("welcome".to_string(), "欢迎使用 rgpui！".to_string());
        i18n.load_translations_map("zh-CN", zh);

        // 初始化虚拟滚动数据
        let virtual_scroll_items: Vec<String> =
            (0..100).map(|i| format!("虚拟滚动项目 #{}", i)).collect();

        // 初始化聊天消息
        let chat_messages = vec![
            Message::text("你好！欢迎使用 rgpui Chat UI"),
            Message::text("这是一个消息列表组件"),
            Message::code_block("rust", "let msg = Message::text(\"hello\");"),
        ];

        Self {
            current_section: NavSection::I18n,
            i18n_locale: "zh-CN".to_string(),
            i18n_manager: i18n,
            config: AppConfig::demo(), // 使用真实默认值
            config_store_path: None,
            theme_mode: ThemeMode::Light,
            block_renderer: BlockRenderer::new(),
            markdown_input: "# 标题\n\n这是 **粗体** 和 *斜体*\n\n## 子标题\n\n- 列表项 1\n- 列表项 2".to_string(),
            virtual_scroll_items,
            source_input: "fn main() {\n    println!(\"Hello\");\n}".to_string(),
            chat_messages,
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

        // 导航栏
        let nav = v_flex()
            .id("nav")
            .w(px(200.0))
            .h_full()
            .bg(rgb(0xf8f9fa))
            .border_r(px(1.0))
            .border_color(rgb(0xe9ecef))
            .p(px(12.0))
            .gap(px(4.0))
            .child(div().text_lg().font_bold().mb(px(8.0)).child("v1.1.0 功能展示"))
            .children(nav_items.into_iter().map(|(section, label)| {
                let is_active = current == section;
                let label = label.to_string();
                div()
                    .id(format!("nav-{:?}", section))
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .text_sm()
                    .when(is_active, |el| el.bg(rgb(0x0078d4)).text_color(rgb(0xffffff)))
                    .when(!is_active, |el| el.hover(|el| el.bg(rgb(0xe9ecef))))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.current_section = section;
                        cx.notify();
                    }))
            }));

        // 内容区域
        let content = match current {
            // ------------------------------------------------------------------
            // I18n 国际化
            // ------------------------------------------------------------------
            NavSection::I18n => v_flex()
                .id("i18n-section")
                .gap(px(16.0))
                .p(px(24.0))
                .child(section_title("I18n 国际化支持"))
                .child(section_desc("动态语言切换，支持变量插值和复数规则"))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .child(
                            Button::new("i18n-zh")
                                .label("中文")
                                .when(i18n_locale == "zh-CN", |b| b.primary())
                                .when(i18n_locale != "zh-CN", |b| b.ghost())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.i18n_locale = "zh-CN".to_string();
                                    this.i18n_manager.set_locale("zh-CN");
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("i18n-en")
                                .label("English")
                                .when(i18n_locale == "en", |b| b.primary())
                                .when(i18n_locale != "en", |b| b.ghost())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.i18n_locale = "en".to_string();
                                    this.i18n_manager.set_locale("en");
                                    cx.notify();
                                })),
                        ),
                )
                .child(card(
                    "翻译结果",
                    &[
                        format!("greeting: {}", greeting),
                        format!("welcome: {}", welcome),
                    ],
                ))
                .child(code_block(
                    "let mut i18n = I18nManager::new(\"zh-CN\");\ni18n.load_translations_map(\"en\", en_translations);\ni18n.set_locale(\"en\");\nprintln!(\"{}\", i18n.t(\"greeting\", &[(\"name\", \"开发者\")]));",
                )),

            // ------------------------------------------------------------------
            // ConfigStore 配置持久化
            // ------------------------------------------------------------------
            NavSection::Config => {
                let config_display = config.clone();
                v_flex()
                    .id("config-section")
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(section_title("ConfigStore 配置持久化"))
                    .child(section_desc("JSON 配置文件的保存、加载和监听"))
                    .child(
                        v_flex()
                            .gap(px(8.0))
                            .child(
                                h_flex().gap(px(8.0)).child("主题: ").child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(4.0))
                                        .rounded(px(4.0))
                                        .bg(rgb(0xe9ecef))
                                        .child(config_display.theme),
                                ),
                            )
                            .child(
                                h_flex().gap(px(8.0)).child("字号: ").child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(4.0))
                                        .rounded(px(4.0))
                                        .bg(rgb(0xe9ecef))
                                        .child(format!("{}", config_display.font_size)),
                                ),
                            )
                            .child(
                                h_flex().gap(px(8.0)).child("自动保存: ").child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(4.0))
                                        .rounded(px(4.0))
                                        .bg(rgb(0xe9ecef))
                                        .child(if config_display.auto_save {
                                            "开启"
                                        } else {
                                            "关闭"
                                        }),
                                ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                Button::new("config-save")
                                    .label("保存配置")
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let temp = tempfile::tempdir().unwrap();
                                        let path = temp.path().join("config.json");
                                        let mut store = ConfigStore::with_path(path.clone());
                                        store.save(&this.config).unwrap();
                                        this.config_store_path = Some(path);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("config-load")
                                    .label("加载配置")
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(path) = &this.config_store_path {
                                            let mut store = ConfigStore::with_path(path.clone());
                                            if let Ok(config) = store.load::<AppConfig>() {
                                                this.config = config;
                                                cx.notify();
                                            }
                                        }
                                    })),
                            ),
                    )
                    .child(code_block(
                        "#[derive(Serialize, Deserialize)]\nstruct AppConfig { theme: String, font_size: u32 }\n\nlet mut store = ConfigStore::with_path(\"config.json\");\nstore.save(&config)?;\nlet loaded: AppConfig = store.load()?;",
                    ))
            }

            // ------------------------------------------------------------------
            // ThemeWatcher 主题热重载
            // ------------------------------------------------------------------
            NavSection::Theme => v_flex()
                .id("theme-section")
                .gap(px(16.0))
                .p(px(24.0))
                .child(section_title("ThemeWatcher 主题热重载"))
                .child(section_desc("监听系统主题变化，支持手动切换"))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .child(
                            Button::new("theme-light")
                                .label("亮色")
                                .when(theme_mode == ThemeMode::Light, |b| b.primary())
                                .when(theme_mode != ThemeMode::Light, |b| b.ghost())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.theme_mode = ThemeMode::Light;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("theme-dark")
                                .label("暗色")
                                .when(theme_mode == ThemeMode::Dark, |b| b.primary())
                                .when(theme_mode != ThemeMode::Dark, |b| b.ghost())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.theme_mode = ThemeMode::Dark;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("theme-system")
                                .label("跟随系统")
                                .when(theme_mode == ThemeMode::System, |b| b.primary())
                                .when(theme_mode != ThemeMode::System, |b| b.ghost())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.theme_mode = ThemeMode::System;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    v_flex()
                        .gap(px(4.0))
                        .child(div().text_sm().font_medium().child("当前主题:"))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .bg(rgb(0xe9ecef))
                                .child(format!("{:?}", theme_mode)),
                        ),
                )
                .child(code_block(
                    "let mut watcher = ThemeWatcher::new();\nwatcher.set_theme(ThemeMode::Dark);\nprintln!(\"{:?}\", watcher.current_theme());\n\nlet mut manager = ThemeManager::new();\nmanager.set_theme(ThemeMode::Dark);",
                )),

            // ------------------------------------------------------------------
            // BlockRender 块级渲染 — 渲染为真实样式元素
            // ------------------------------------------------------------------
            NavSection::BlockRender => {
                let blocks = block_renderer.parse_markdown(&markdown_input);

                v_flex()
                    .id("block-render-section")
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(section_title("BlockRender 块级渲染"))
                    .child(section_desc("Markdown 文本的块级元素解析与渲染"))
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(div().text_sm().font_medium().child("解析结果:"))
                            .children(blocks.into_iter().map(|block| {
                                render_block_element(&block)
                            })),
                    )
                    .child(code_block(
                        "let renderer = BlockRenderer::new();\nlet blocks = renderer.parse_markdown(\"# 标题\\n\\n段落内容\");\nfor block in blocks {\n    match block.block_type {\n        BlockType::Heading(1) => { /* 渲染 h1 */ }\n        BlockType::Paragraph => { /* 渲染段落 */ }\n        _ => {}\n    }\n}",
                    ))
            }

            // ------------------------------------------------------------------
            // VirtualScroll 虚拟滚动 — 可滚动
            // ------------------------------------------------------------------
            NavSection::VirtualScroll => {
                let item_count = virtual_scroll_items.len();

                v_flex()
                    .id("virtual-scroll-section")
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(section_title("VirtualScroll 虚拟滚动"))
                    .child(section_desc("仅渲染可视区域内的列表项，支持大量数据"))
                    .child(div().text_sm().font_medium().child(format!("数据量: {} 项", item_count)))
                    .child(
                        div()
                            .id("virtual-scroll-container")
                            .h(px(300.0))
                            .rounded(px(6.0))
                            .border(px(1.0))
                            .border_color(rgb(0xe9ecef))
                            .overflow_scroll()
                    .child(
                        // 内容高度 = item_count * 32px，产生滚动
                        div()
                            .id("virtual-scroll-content")
                            .w_full()
                            .children((0..item_count).map(|i| {
                                div()
                                    .id(format!("vs-item-{}", i))
                                    .w_full()
                                    .h(px(32.0))
                                    .flex()
                                    .items_center()
                                    .px(px(12.0))
                                    .text_sm()
                                    .border_b(px(1.0))
                                    .border_color(rgb(0xf0f0f0))
                                    .when(i % 2 == 0, |el| el.bg(rgb(0xf8f9fa)))
                                    .child(format!("{} - 项目 #{}", i, i))
                            })),
                            ),
                    )
                    .child(card(
                        "配置信息",
                        &[
                            "direction: Vertical".to_string(),
                            "buffer_size: 5".to_string(),
                            "estimated_item_height: 32px".to_string(),
                            "支持 overflow_scroll 自动滚动".to_string(),
                        ],
                    ))
                    .child(code_block(
                        "let config = VirtualScrollConfig {\n    direction: VirtualScrollDirection::Vertical,\n    buffer_size: 5,\n    estimated_item_height: 32.0,\n};\nVirtualScroll::new(state).config(config)",
                    ))
            }

            // ------------------------------------------------------------------
            // SourceMap 源码映射
            // ------------------------------------------------------------------
            NavSection::SourceMap => {
                let source_map = SourceMap::new("example.rs", &source_input);
                let line_count = source_map.line_count();

                let lines: Vec<String> = (1..=line_count.min(5))
                    .filter_map(|i| source_map.get_line(i).map(|l| format!("{}: {}", i, l)))
                    .collect();

                v_flex()
                    .id("source-map-section")
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(section_title("SourceMap 源码映射"))
                    .child(section_desc("源码行号/列号定位与双向映射"))
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(div().text_sm().font_medium().child("源码:"))
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(6.0))
                                    .rounded(px(6.0))
                                    .bg(rgb(0x282c34))
                                    .text_sm()
                                    .text_color(rgb(0xabb2bf))
                                    .child(source_input),
                            ),
                    )
                    .child(card(
                        "解析结果",
                        &[
                            format!("总行数: {}", line_count),
                            format!(
                                "搜索 'println': {} 处",
                                source_map.search("println").len()
                            ),
                        ],
                    ))
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(div().text_sm().font_medium().child("行内容:"))
                            .children(lines.into_iter().map(|l| {
                                div()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .text_sm()
                                    .child(l)
                            })),
                    )
                    .child(code_block(
                        "let source_map = SourceMap::new(\"main.rs\", source);\nprintln!(\"行数: {}\", source_map.line_count());\nif let Some(loc) = source_map.get_location(0) {\n    println!(\"位置: 行 {} 列 {}\", loc.line, loc.column);\n}",
                    ))
            }

            // ------------------------------------------------------------------
            // TabDrag 标签拖拽排序
            // ------------------------------------------------------------------
            NavSection::TabDrag => {
                let tabs = vec![("main.rs", true), ("lib.rs", true), ("mod.rs", false)];

                v_flex()
                    .id("tab-drag-section")
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(section_title("TabDrag 标签拖拽排序"))
                    .child(section_desc("支持鼠标拖拽排序的标签页组件"))
                    .child(
                        h_flex()
                            .id("tab-bar")
                            .gap(px(2.0))
                            .bg(rgb(0xf8f9fa))
                            .p(px(4.0))
                            .rounded(px(6.0))
                            .children(tabs.into_iter().enumerate().map(|(i, (name, closable))| {
                                let mut tab = h_flex()
                                    .id(format!("tab-{}", i))
                                    .gap(px(8.0))
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0xffffff))
                                    .border(px(1.0))
                                    .border_color(rgb(0xe9ecef))
                                    .text_sm()
                                    .child(name.to_string());
                                if closable {
                                    tab = tab.child(
                                        div().text_xs().text_color(rgb(0x999)).child("x"),
                                    );
                                }
                                tab
                            })),
                    )
                    .child(
                        div()
                            .mt(px(8.0))
                            .p(px(8.0))
                            .rounded(px(6.0))
                            .bg(rgb(0xfff3cd))
                            .text_sm()
                            .text_color(rgb(0x856404))
                            .child("提示: 拖拽排序需要接入 TabDragDrop 组件，此处为样式展示"),
                    )
                    .child(card(
                        "Tab 配置",
                        &[
                            "支持拖拽排序".to_string(),
                            "可设置是否可关闭".to_string(),
                            "支持关闭事件回调".to_string(),
                        ],
                    ))
                    .child(code_block(
                        "let tabs = vec![\n    TabItem { title: \"main.rs\", id: \"t1\", closable: true },\n    TabItem { title: \"lib.rs\", id: \"t2\", closable: true },\n];\nTabDragDrop::new(state).items(tabs)",
                    ))
            }

            // ------------------------------------------------------------------
            // FpsHud 性能监控
            // ------------------------------------------------------------------
            NavSection::FpsHud => {
                let fps = 60.0;
                let frame_time = 16.67;
                let color = if fps >= 55.0 {
                    rgb(0x28a745)
                } else if fps >= 30.0 {
                    rgb(0xffc107)
                } else {
                    rgb(0xdc3545)
                };

                v_flex()
                    .id("fps-hud-section")
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(section_title("FpsHud 性能监控"))
                    .child(section_desc("实时 FPS/CPU/GPU 监控显示"))
                    .child(
                        h_flex()
                            .gap(px(24.0))
                            .child(
                                v_flex()
                                    .gap(px(4.0))
                                    .child(div().text_xs().text_color(rgb(0x666)).child("FPS"))
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_bold()
                                            .text_color(color)
                                            .child(format!("{:.0}", fps)),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap(px(4.0))
                                    .child(
                                        div().text_xs().text_color(rgb(0x666)).child("帧时间"),
                                    )
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_bold()
                                            .child(format!("{:.2}ms", frame_time)),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap(px(4.0))
                                    .child(div().text_xs().text_color(rgb(0x666)).child("状态"))
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_bold()
                                            .text_color(color)
                                            .child("流畅"),
                                    ),
                            ),
                    )
                    .child(card(
                        "颜色规则",
                        &[
                            "FPS >= 55: 绿色 (流畅)".to_string(),
                            "FPS >= 30: 黄色 (卡顿)".to_string(),
                            "FPS < 30: 红色 (严重卡顿)".to_string(),
                        ],
                    ))
                    .child(code_block(
                        "let state = cx.new(FpsHudState::new);\nFpsHud::new(state)\n    .position(Point::new(px(10.0), px(10.0)))\n    .show_cpu(true)\n    .show_gpu(true)",
                    ))
            }

            // ------------------------------------------------------------------
            // ChatUI 聊天组件
            // ------------------------------------------------------------------
            NavSection::Chat => {
                v_flex()
                    .id("chat-section")
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(section_title("ChatUI 聊天组件"))
                    .child(section_desc("支持消息分组、多类型的聊天界面"))
                    .child(
                        v_flex()
                            .id("chat-messages")
                            .gap(px(8.0))
                            .h(px(250.0))
                            .rounded(px(6.0))
                            .border(px(1.0))
                            .border_color(rgb(0xe9ecef))
                            .p(px(12.0))
                            .overflow_scroll()
                            .children(chat_messages.iter().map(|msg| {
                                let (bg, text_color) = match &msg.content {
                                    MessageType::Text(_) => (rgb(0xf8f9fa), rgb(0x333333)),
                                    MessageType::CodeBlock { .. } => {
                                        (rgb(0x282c34), rgb(0xabb2bf))
                                    }
                                };
                                let content_text = match &msg.content {
                                    MessageType::Text(s) => s.clone(),
                                    MessageType::CodeBlock { code, .. } => code.clone(),
                                };
                                div()
                                    .id(msg.id.clone())
                                    .px(px(10.0))
                                    .py(px(6.0))
                                    .rounded(px(6.0))
                                    .bg(bg)
                                    .text_sm()
                                    .text_color(text_color)
                                    .child(content_text)
                            })),
                    )
                    .child(card(
                        "消息类型",
                        &[
                            "MessageType::Text(String) — 纯文本".to_string(),
                            "MessageType::CodeBlock { language, code } — 代码块".to_string(),
                            "Message::text(\"...\") — 便捷构造".to_string(),
                            "Message::code_block(\"rust\", \"...\") — 代码构造".to_string(),
                        ],
                    ))
                    .child(code_block(
                        "let msg = Message::text(\"你好\");\nlet code = Message::code_block(\"rust\", \"fn main() {}\");\nlet group = MessageGroup { sender: \"用户\", messages: vec![msg] };",
                    ))
            }
        };

        h_flex()
            .id("showcase-app")
            .size_full()
            .child(nav)
            .child(content)
    }
}

// ============================================================================
// 辅助组件
// ============================================================================

/// 将 BlockElement 渲染为真实的 rgpui 样式元素。
fn render_block_element(block: &BlockElement) -> impl IntoElement + use<> {
    match &block.block_type {
    BlockType::Heading(level) => {
        let styled_div = div()
            .id(format!("heading-{}", block.content))
            .mb(px(4.0))
            .text_color(rgb(0x1a1a1a));
        let styled_div = match level {
            1 => styled_div.text_2xl().font_bold(),
            2 => styled_div.text_xl().font_semibold(),
            3 => styled_div.text_lg().font_medium(),
            _ => styled_div.text_base(),
        };
        styled_div.child(block.content.clone())
    }
        BlockType::Paragraph => div()
            .id(format!("para-{}", block.content.chars().take(10).collect::<String>()))
            .mb(px(8.0))
            .text_sm()
            .text_color(rgb(0x333333))
            .child(block.content.clone()),
        BlockType::CodeBlock => {
            let lang = block.attributes.get("language").map(|s| s.as_str()).unwrap_or("");
            div()
                .id(format!("code-{}", lang))
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
        }
        BlockType::Blockquote => div()
            .id(format!("quote-{}", block.content.chars().take(10).collect::<String>()))
            .mb(px(8.0))
            .pl(px(12.0))
            .border_l(px(3.0))
            .border_color(rgb(0x0078d4))
            .text_sm()
            .text_color(rgb(0x666666))
            .italic()
            .child(block.content.clone()),
        BlockType::HorizontalRule => div()
            .id("hr")
            .my(px(12.0))
            .h(px(1.0))
            .bg(rgb(0xe9ecef)),
        BlockType::List => div()
            .id("list")
            .mb(px(8.0))
            .pl(px(16.0))
            .text_sm()
            .child(block.content.clone()),
        BlockType::Image => {
            let alt = block.attributes.get("alt").map(|s| s.as_str()).unwrap_or("");
            div()
                .id("image")
                .mb(px(8.0))
                .text_sm()
                .text_color(rgb(0x666))
                .child(format!("[图片: {}]", alt))
        }
        BlockType::Table => div()
            .id("table")
            .mb(px(8.0))
            .text_sm()
            .child(block.content.clone()),
        BlockType::Custom(name) => div()
            .id(format!("custom-{}", name))
            .mb(px(8.0))
            .text_sm()
            .child(format!("[自定义块: {}]", name)),
    }
}

/// 区域标题。
fn section_title(text: &str) -> impl IntoElement {
    div().text_xl().font_bold().child(text.to_string())
}

/// 区域描述。
fn section_desc(text: &str) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(rgb(0x666666))
        .child(text.to_string())
}

/// 信息卡片。
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

/// 代码块。
fn code_block(code: &str) -> impl IntoElement {
    div()
        .rounded(px(8.0))
        .bg(rgb(0x282c34))
        .p(px(12.0))
        .text_sm()
        .text_color(rgb(0xabb2bf))
        .child(code.to_string())
}

// ============================================================================
// 入口
// ============================================================================

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
