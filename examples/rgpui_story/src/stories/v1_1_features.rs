//! v1.1.0 新功能示例：国际化、配置持久化、主题热重载、块级渲染、
//! 虚拟滚动、源码映射、Tab 拖拽、状态栏、FPS 监控、聊天组件。

use rgpui::components::status_bar::LspStatus;
use rgpui::prelude::*;
use rgpui::{
    AnyElement, Button, ButtonVariants as _, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, Window, div, h_flex, px, rgb, v_flex,
};

use super::StoryItem;

/// v1.1.0 新功能故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![
        StoryItem {
            title: "I18n 国际化",
            build: |window, cx| cx.new(|cx| I18nStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "ConfigStore 配置",
            build: |window, cx| cx.new(|cx| ConfigStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "ThemeWatcher 主题",
            build: |window, cx| cx.new(|cx| ThemeStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "BlockRender 块渲染",
            build: |window, cx| cx.new(|cx| BlockRenderStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "VirtualScroll 虚拟滚动",
            build: |window, cx| cx.new(|cx| VirtualScrollStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "SourceMap 源码映射",
            build: |window, cx| cx.new(|cx| SourceMapStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "TabDrag 标签拖拽",
            build: |window, cx| cx.new(|cx| TabDragStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "StatusBar 状态栏",
            build: |window, cx| cx.new(|cx| StatusBarStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "FpsHud 性能监控",
            build: |window, cx| cx.new(|cx| FpsHudStory::new(window, cx)).into(),
        },
        StoryItem {
            title: "ChatUI 聊天",
            build: |window, cx| cx.new(|cx| ChatStory::new(window, cx)).into(),
        },
    ]
}

// ============================================================================
// I18n 国际化
// ============================================================================

struct I18nStory {
    locale: String,
    manager: I18nManager,
}

impl I18nStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let mut manager = I18nManager::new("zh-CN");
        let mut en = std::collections::HashMap::new();
        en.insert("greeting".to_string(), "Hello, {name}!".to_string());
        en.insert("welcome".to_string(), "Welcome to rgpui!".to_string());
        manager.load_translations_map("en", en);
        let mut zh = std::collections::HashMap::new();
        zh.insert("greeting".to_string(), "你好，{name}！".to_string());
        zh.insert("welcome".to_string(), "欢迎使用 rgpui！".to_string());
        manager.load_translations_map("zh-CN", zh);
        Self {
            locale: "zh-CN".to_string(),
            manager,
        }
    }
}

impl Render for I18nStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale.clone();
        let greeting = self.manager.t("greeting", &[("name", "开发者")]);
        let welcome = self.manager.t("welcome", &[]);

        v_flex()
            .id("i18n-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("I18n 国际化支持"))
            .child(section_desc("动态语言切换，支持变量插值和复数规则"))
            .child(
                h_flex().gap(px(8.0)).child(
                    Button::new("i18n-zh")
                        .label("中文")
                        .when(locale == "zh-CN", |b| b.primary())
                        .when(locale != "zh-CN", |b| b.ghost())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.locale = "zh-CN".to_string();
                            this.manager.set_locale("zh-CN");
                            cx.notify();
                        })),
                ).child(
                    Button::new("i18n-en")
                        .label("English")
                        .when(locale == "en", |b| b.primary())
                        .when(locale != "en", |b| b.ghost())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.locale = "en".to_string();
                            this.manager.set_locale("en");
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
                "let mut i18n = I18nManager::new(\"zh-CN\");\ni18n.set_locale(\"en\");\nprintln!(\"{}\", i18n.t(\"greeting\", &[(\"name\", \"开发者\")]));",
            ))
    }
}

// ============================================================================
// ConfigStore 配置持久化
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
struct AppConfig {
    theme: String,
    font_size: u32,
    auto_save: bool,
}

struct ConfigStory {
    config: AppConfig,
    saved_path: Option<std::path::PathBuf>,
}

impl ConfigStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            config: AppConfig {
                theme: "dark".to_string(),
                font_size: 14,
                auto_save: true,
            },
            saved_path: None,
        }
    }
}

impl Render for ConfigStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = self.config.clone();

        v_flex()
            .id("config-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("ConfigStore 配置持久化"))
            .child(section_desc("JSON 配置文件的保存、加载和监听"))
            .child(
                v_flex()
                    .gap(px(4.0))
                    .child(h_flex()
                        .gap(px(8.0))
                        .child("主题: ")
                        .child(div().px(px(8.0)).py(px(4.0)).rounded(px(4.0)).bg(rgb(0xe9ecef)).child(config.theme)))
                    .child(h_flex()
                        .gap(px(8.0))
                        .child("字号: ")
                        .child(div().px(px(8.0)).py(px(4.0)).rounded(px(4.0)).bg(rgb(0xe9ecef)).child(format!("{}", config.font_size))))
                    .child(h_flex()
                        .gap(px(8.0))
                        .child("自动保存: ")
                        .child(div().px(px(8.0)).py(px(4.0)).rounded(px(4.0)).bg(rgb(0xe9ecef)).child(if config.auto_save { "开启" } else { "关闭" }))),
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
                                this.saved_path = Some(path);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("config-load")
                            .label("加载配置")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(path) = &this.saved_path {
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
}

// ============================================================================
// ThemeWatcher 主题热重载
// ============================================================================

struct ThemeStory {
    mode: ThemeMode,
}

impl ThemeStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            mode: ThemeMode::Light,
        }
    }
}

impl Render for ThemeStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = self.mode.clone();

        v_flex()
            .id("theme-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("ThemeWatcher 主题热重载"))
            .child(section_desc("监听系统主题变化，支持手动切换"))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .child(
                        Button::new("theme-light")
                            .label("亮色")
                            .when(mode == ThemeMode::Light, |b| b.primary())
                            .when(mode != ThemeMode::Light, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = ThemeMode::Light;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("theme-dark")
                            .label("暗色")
                            .when(mode == ThemeMode::Dark, |b| b.primary())
                            .when(mode != ThemeMode::Dark, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = ThemeMode::Dark;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("theme-system")
                            .label("跟随系统")
                            .when(mode == ThemeMode::System, |b| b.primary())
                            .when(mode != ThemeMode::System, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = ThemeMode::System;
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
                            .child(format!("{:?}", mode)),
                    ),
            )
            .child(code_block(
                "let mut watcher = ThemeWatcher::new();\nwatcher.set_theme(ThemeMode::Dark);",
            ))
    }
}

// ============================================================================
// BlockRender 块级渲染
// ============================================================================

struct BlockRenderStory {
    renderer: BlockRenderer,
    input: String,
}

impl BlockRenderStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            renderer: BlockRenderer::new(),
            input: "# 标题\n\n这是 **粗体** 和 *斜体*\n\n## 子标题\n\n- 列表项 1\n- 列表项 2"
                .to_string(),
        }
    }
}

impl Render for BlockRenderStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let blocks = self.renderer.parse_markdown(&self.input);

        v_flex()
            .id("block-render-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("BlockRender 块级渲染"))
            .child(section_desc("Markdown 文本的块级元素解析与渲染"))
            .child(
                v_flex()
                    .gap(px(4.0))
                    .child(div().text_sm().font_medium().child("解析结果:"))
                    .children(blocks.into_iter().map(|block| render_block_element(&block))),
            )
            .child(code_block(
                "let renderer = BlockRenderer::new();\nlet blocks = renderer.parse_markdown(\"# 标题\\n\\n**粗体** 和 *斜体*\");",
            ))
    }
}

// ============================================================================
// VirtualScroll 虚拟滚动
// ============================================================================

struct VirtualScrollStory;

impl VirtualScrollStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for VirtualScrollStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let item_count = 100;

        v_flex()
            .id("virtual-scroll-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("VirtualScroll 虚拟滚动"))
            .child(section_desc("仅渲染可视区域内的列表项，支持大量数据"))
            .child(div().text_sm().font_medium().child(format!("数据量: {} 项", item_count)))
            .child(
                div()
                    .id("vs-container")
                    .h(px(300.0))
                    .rounded(px(6.0))
                    .border(px(1.0))
                    .border_color(rgb(0xe9ecef))
                    .overflow_scroll()
                    .child(
                        div()
                            .id("vs-content")
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
                ],
            ))
            .child(code_block(
                "let config = VirtualScrollConfig { direction: Vertical, buffer_size: 5, estimated_item_height: 32.0 };\nVirtualScroll::new(state).config(config)",
            ))
    }
}

// ============================================================================
// SourceMap 源码映射
// ============================================================================

struct SourceMapStory;

impl SourceMapStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for SourceMapStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let source = "fn main() {\n    println!(\"Hello\");\n}";
        let source_map = SourceMap::new("example.rs", source);
        let line_count = source_map.line_count();
        let lines: Vec<String> = (1..=line_count.min(5))
            .filter_map(|i| source_map.get_line(i).map(|l| format!("{}: {}", i, l)))
            .collect();

        v_flex()
            .id("source-map-story")
            .gap(px(8.0))
            .p(px(16.0))
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
                            .child(source.to_string()),
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
                        div().px(px(8.0)).py(px(2.0)).text_sm().child(l)
                    })),
            )
            .child(code_block(
                "let source_map = SourceMap::new(\"main.rs\", source);\nprintln!(\"行数: {}\", source_map.line_count());",
            ))
    }
}

// ============================================================================
// TabDrag 标签拖拽
// ============================================================================

struct TabDragStory {
    state: Entity<TabDragState>,
}

impl TabDragStory {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| {
            let mut state = TabDragState::default();
            state.enabled = true;
            state.tabs = vec![
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
            ];
            state
        });
        Self { state }
    }
}

impl Render for TabDragStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.state.read(cx).tabs.clone();
        let tab_drag = TabDragDrop::new(self.state.clone()).on_reorder(|_tabs, _window, _cx| {
            println!("Tab 重新排序完成");
        });

        v_flex()
            .id("tab-drag-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("TabDrag 标签拖拽排序"))
            .child(section_desc("拖拽 Tab 进行排序，支持 on_drag/drag_over/on_drop 事件"))
            .child(tab_drag)
            .child(
                h_flex()
                    .gap(px(8.0))
                    .child(
                        Button::new("move-left")
                            .label("左移")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let state = this.state.read(cx);
                                if let Some(first) = state.tabs.iter().position(|t| t.closable) {
                                    if first > 0 {
                                        let (from, to) = (first, first - 1);
                                        let _ = state;
                                        this.state.update(cx, |s, _| s.move_tab(from, to));
                                        cx.notify();
                                    }
                                }
                            })),
                    )
                    .child(
                        Button::new("move-right")
                            .label("右移")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let state = this.state.read(cx);
                                if let Some(first) = state.tabs.iter().position(|t| t.closable) {
                                    if first < state.tabs.len() - 1 {
                                        let (from, to) = (first, first + 1);
                                        let _ = state;
                                        this.state.update(cx, |s, _| s.move_tab(from, to));
                                        cx.notify();
                                    }
                                }
                            })),
                    )
                    .child(
                        Button::new("reset-tabs")
                            .label("重置")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |s, _| {
                                    s.tabs = vec![
                                        TabItem { title: "main.rs".to_string(), id: "t1".to_string(), closable: true },
                                        TabItem { title: "lib.rs".to_string(), id: "t2".to_string(), closable: true },
                                        TabItem { title: "mod.rs".to_string(), id: "t3".to_string(), closable: false },
                                        TabItem { title: "utils.rs".to_string(), id: "t4".to_string(), closable: true },
                                    ];
                                });
                                cx.notify();
                            })),
                    ),
            )
            .child(card(
                "拖拽状态",
                &[
                    format!("Tab 数量: {}", tabs.len()),
                    "拖拽 Tab 可进行排序，蓝色线条指示目标位置".to_string(),
                ],
            ))
            .child(code_block(
                "let drag_state = cx.new(|_| TabDragState::default());\nTabDragDrop::new(drag_state)\n    .on_reorder(|tabs, window, cx| { /* 处理排序 */ })",
            ))
    }
}

// ============================================================================
// StatusBar 状态栏
// ============================================================================

#[derive(Debug, Clone)]
struct StatusBarState {
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

impl Default for StatusBarState {
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

struct StatusBarStory {
    state: StatusBarState,
}

impl StatusBarStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            state: StatusBarState::default(),
        }
    }
}

impl Render for StatusBarStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sb = self.state.clone();

        v_flex()
            .id("status-bar-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("StatusBar 状态栏"))
            .child(section_desc("显示编辑器状态信息，支持动态修改"))
            .child(
                h_flex()
                    .gap(px(8.0))
                    .flex_wrap()
                    .child(Button::new("sb-line-up").label("行号+1").ghost().on_click(cx.listener(|this, _, _, cx| { this.state.line += 1; cx.notify(); })))
                    .child(Button::new("sb-line-down").label("行号-1").ghost().on_click(cx.listener(|this, _, _, cx| { this.state.line = this.state.line.saturating_sub(1).max(1); cx.notify(); })))
                    .child(Button::new("sb-col-up").label("列号+5").ghost().on_click(cx.listener(|this, _, _, cx| { this.state.column += 5; cx.notify(); })))
                    .child(Button::new("sb-lang").label("切换语言").ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.state.language = if this.state.language == "Rust" { "TypeScript".to_string() } else { "Rust".to_string() };
                        cx.notify();
                    })))
                    .child(Button::new("sb-lsp").label("切换LSP").ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.state.lsp_status = match this.state.lsp_status {
                            LspStatus::Connected => LspStatus::Disconnected,
                            LspStatus::Disconnected => LspStatus::Initializing,
                            LspStatus::Initializing => LspStatus::Connected,
                            LspStatus::Error => LspStatus::Connected,
                        };
                        cx.notify();
                    })))
                    .child(Button::new("sb-error").label("错误+1").ghost().on_click(cx.listener(|this, _, _, cx| { this.state.error_count += 1; cx.notify(); })))
                    .child(Button::new("sb-warn").label("警告+1").ghost().on_click(cx.listener(|this, _, _, cx| { this.state.warning_count += 1; cx.notify(); })))
                    .child(Button::new("sb-reset").label("重置").primary().on_click(cx.listener(|this, _, _, cx| { this.state = StatusBarState::default(); cx.notify(); }))),
            )
            .child(card(
                "当前状态",
                &[
                    format!("行: {}, 列: {}", sb.line, sb.column),
                    format!("语言: {}", sb.language),
                    format!("编码: {}", sb.encoding),
                    format!("LSP: {:?} ({})", sb.lsp_status, sb.lsp_server_name),
                    format!("错误: {}, 警告: {}", sb.error_count, sb.warning_count),
                    format!("Git: {}", sb.git_branch),
                ],
            ))
            .child(render_status_bar_preview(&sb))
            .child(code_block(
                "let status = cx.new(|_| StatusBarState {\n    line: 42, column: 15,\n    language: \"Rust\".into(),\n    lsp_status: LspStatus::Connected,\n    ..Default::default()\n});\nStatusBar::new(status)",
            ))
    }
}

fn render_status_bar_preview(sb: &StatusBarState) -> AnyElement {
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
        .id("status-bar-preview")
        .w_full()
        .h(px(28.0))
        .items_center()
        .justify_between()
        .px(px(12.0))
        .bg(rgb(0xf0f0f0))
        .border(px(1.0))
        .border_color(rgb(0xe0e0e0))
        .rounded(px(4.0))
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
        .into_any_element()
}

// ============================================================================
// FpsHud 性能监控
// ============================================================================

struct FpsHudStory;

impl FpsHudStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for FpsHudStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
            .id("fps-hud-story")
            .gap(px(8.0))
            .p(px(16.0))
            .child(section_title("FpsHud 性能监控"))
            .child(section_desc("实时 FPS/CPU/GPU 监控显示"))
            .child(
                h_flex()
                    .gap(px(24.0))
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(div().text_xs().text_color(rgb(0x666)).child("FPS"))
                            .child(div().text_3xl().font_bold().text_color(color).child(format!("{:.0}", fps))),
                    )
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(div().text_xs().text_color(rgb(0x666)).child("帧时间"))
                            .child(div().text_3xl().font_bold().child(format!("{:.2}ms", frame_time))),
                    )
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(div().text_xs().text_color(rgb(0x666)).child("状态"))
                            .child(div().text_3xl().font_bold().text_color(color).child("流畅")),
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
                "let state = cx.new(FpsHudState::new);\nFpsHud::new(state).position(Point::new(px(10.0), px(10.0))).show_cpu(true)",
            ))
    }
}

// ============================================================================
// ChatUI 聊天组件
// ============================================================================

struct ChatStory;

impl ChatStory {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for ChatStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let messages = [
            Message::text("你好！欢迎使用 rgpui Chat UI"),
            Message::text("这是一个消息列表组件"),
            Message::code_block("rust", "let msg = Message::text(\"hello\");"),
        ];

        v_flex()
            .id("chat-story")
            .gap(px(8.0))
            .p(px(16.0))
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
                    .children(messages.iter().map(|msg| {
                        let (bg, tc) = match &msg.content {
                            MessageType::Text(_) => (rgb(0xf8f9fa), rgb(0x333333)),
                            MessageType::CodeBlock { .. } => (rgb(0x282c34), rgb(0xabb2bf)),
                        };
                        let ct = match &msg.content {
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
                            .text_color(tc)
                            .child(ct)
                    })),
            )
            .child(card(
                "消息类型",
                &[
                    "MessageType::Text(String) — 纯文本".to_string(),
                    "MessageType::CodeBlock { language, code } — 代码块".to_string(),
                ],
            ))
            .child(code_block(
                "let msg = Message::text(\"你好\");\nlet code = Message::code_block(\"rust\", \"fn main() {}\");",
            ))
    }
}

// ============================================================================
// 辅助函数
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
