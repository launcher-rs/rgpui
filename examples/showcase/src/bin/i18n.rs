//! 国际化（i18n）示例。
//!
//! 演示 `I18nManager` 的两种用法：
//! - **全局管理器**：`cx.set_global(i18n)` 注册后，用 `I18nText::new(key).translate_global(cx)`
//!   在渲染时取翻译；切换语言调 `cx.update_global::<I18nManager, _>(|m, _| m.set_locale("en"))`。
//! - **局部管理器**：直接持有一个 `I18nManager`，用 `manager.t(key, &[("name", value)])`
//!   取值（支持 `{name}` 变量插值），另有复数规则 `PluralRule`。
//!
//! 运行：
//!
//! ```text
//! cargo run -p showcase --bin i18n
//! ```

#![cfg_attr(target_family = "wasm", no_main)]

use std::collections::HashMap;

use rgpui::i18n::{I18nManager, I18nText, PluralRule};
use rgpui::{
    App, Bounds, Context, IntoElement, ParentElement, Render, Window, WindowBounds, WindowOptions,
    div, h_flex, prelude::*, px, rgb, size, v_flex,
};
use rgpui_platform::application;

/// 示例根视图：局部管理器 + 语言回显。
struct I18nDemo {
    /// 局部管理器（不放进全局，演示直接持有用法）。
    local: I18nManager,
}

impl I18nDemo {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        // 构造一个带中英文的局部管理器。
        let mut local = I18nManager::new("zh-CN");
        local.load_translations_map(
            "zh-CN",
            HashMap::from([
                ("greeting".to_string(), "你好，{name}！".to_string()),
                ("welcome".to_string(), "欢迎使用 rgpui！".to_string()),
            ]),
        );
        local.load_translations_map(
            "en",
            HashMap::from([
                ("greeting".to_string(), "Hello, {name}!".to_string()),
                ("welcome".to_string(), "Welcome to rgpui!".to_string()),
            ]),
        );
        Self { local }
    }
}

impl Render for I18nDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 局部管理器取值：变量插值。
        let greeting = self.local.t("greeting", &[("name", "开发者")]);
        let welcome = self.local.t("welcome", &[]);
        // 复数规则：1 用单数、5 用复数。
        let one = PluralRule::new(1, "item", "items");
        let many = PluralRule::new(5, "item", "items");
        let current = self.local.current_locale().to_string();

        v_flex()
            .id("i18n-demo")
            .size_full()
            .gap(px(16.0))
            .p(px(24.0))
            .bg(rgb(0xffffff))
            .child(div().text_2xl().child("国际化（i18n）"))
            // 1. 全局管理器：切换语言 + I18nText::translate_global。
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(div().text_lg().child("1. 全局管理器（cx.set_global）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("i18n-locale-zh")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(rgb(0x0078d4))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("中文")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.update_global::<I18nManager, _>(|m, _| {
                                            m.set_locale("zh-CN");
                                        });
                                    })),
                            )
                            .child(
                                div()
                                    .id("i18n-locale-en")
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(rgb(0x107c10))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("English")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.update_global::<I18nManager, _>(|m, _| {
                                            m.set_locale("en");
                                        });
                                    })),
                            ),
                    )
                    .child(
                        div().text_sm().child(
                            I18nText::new("greeting")
                                .with_arg("name", "1.5")
                                .translate_global(cx),
                        ),
                    ),
            )
            // 2. 局部管理器：直接持有 + t(key, args) + 复数。
            .child(
                v_flex()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(div().text_lg().child("2. 局部管理器（I18nManager）"))
                    .child(div().text_sm().child(format!("当前语言：{current}")))
                    .child(div().text_sm().child(format!("greeting：{greeting}")))
                    .child(div().text_sm().child(format!("welcome：{welcome}")))
                    .child(div().text_sm().child(format!(
                        "复数：{} / {}",
                        one.translate(&self.local),
                        many.translate(&self.local)
                    ))),
            )
            // 用法速查。
            .child(
                div()
                    .rounded_md()
                    .bg(rgb(0x282c34))
                    .p(px(12.0))
                    .text_sm()
                    .text_color(rgb(0xabb2bf))
                    .child(
                        "let mut i18n = I18nManager::new(\"zh-CN\");\n\
                         i18n.load_translations_map(\"en\", map);\n\
                         i18n.set_locale(\"en\");\n\
                         i18n.t(\"greeting\", &[(\"name\", \"rgpui\")]);\n\n\
                         // 全局：\n\
                         cx.set_global(i18n);\n\
                         I18nText::new(\"greeting\").with_arg(\"name\", \"rgpui\").translate_global(cx);",
                    ),
            )
    }
}

/// 启动示例：注册全局 i18n 管理器。
fn run_example() {
    application().run(|cx: &mut App| {
        // 全局管理器：中英文各一条。
        let mut i18n = I18nManager::new("zh-CN");
        i18n.load_translations_map(
            "zh-CN",
            HashMap::from([("greeting".to_string(), "你好，{name}！".to_string())]),
        );
        i18n.load_translations_map(
            "en",
            HashMap::from([("greeting".to_string(), "Hello, {name}!".to_string())]),
        );
        cx.set_global(i18n);

        let bounds = Bounds::centered(None, size(px(720.0), px(560.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| I18nDemo::new(window, cx)),
        );
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
