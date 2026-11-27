//! 文本 / 国际化 / 富文本 示例集合。
//!
//! 用法：cargo run -p showcase --bin text_gallery -- <demo>
//! 可用 demo：i18n / intl_text / markdown / markdown_stream / markdown_selection / rich_text / rich_clipboard / text / text_layout / text_wrapper
//! 不带参数时默认运行 markdown，传 --list 打印清单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    AnyView, App, Bounds, Button, ButtonVariants, Context, IntoElement, ParentElement, Render,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

mod i18n {
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
        App, Bounds, Context, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

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
                    ("greeting".to_string(), "你好，{name}！".to_string()),
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

    pub fn run_example() {
        application().run(|cx: &mut App| {
            // 全局管理器：中英文各一条。
            let mut i18n = I18nManager::new("zh-CN");
            i18n.load_translations_map(
                "zh-CN",
                HashMap::from([("greeting".to_string(), "你好，{name}！".to_string())]),
            );
            i18n.load_translations_map(
                "en",
                HashMap::from([("greeting".to_string(), "你好，{name}！".to_string())]),
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| I18nDemo::new(window, cx)).into()
    }
}

mod intl_text {
    //! 国际文本排版示例（bidi + 东亚断行）
    //!
    //! 演示两个核心模块：
    //!
    //! - `rgpui::bidi`：把「段落属于 LTR 还是 RTL」解析出来（HTML `dir=auto` 口径），
    //!   `TextAlign::Start` / `End` 据此对齐 —— 同一份布局在希伯来 / 阿拉伯文本下自动
    //!   靠右 / 靠左，不必让应用自己判断。
    //! - `rgpui::line_break`：CJK 逐字换行时的行首 / 行末禁则（禁则 / kinsoku）与胶连
    //!   字符规则 —— `。` 不再跑到行首、`「` 不再留在行末。
    //! - `rgpui::i18n`：语言标签 → 默认方向（`TextDirection::for_locale`），以及语言包
    //!   缺 key 审计（`I18nManager::validate`）。
    //!
    //! 运行：`cargo run -p showcase --bin intl_text`
    //!
    //! 说明：字形重排（UBA 完整实现）属于平台 shaping 的职责 —— Web / cosmic-text 后端
    //! 内部已用 `unicode-bidi` 完成；Windows DirectWrite 后端把整行交给 `IDWriteTextLayout`，
    //! 由 DirectWrite 自行重排。核心层负责的是段落方向与断行规则。

    #![cfg_attr(target_family = "wasm", no_main)]

    use std::collections::HashMap;

    use rgpui::bidi::{TextDirection, is_rtl_char, strong_direction};
    use rgpui::i18n::I18nManager;
    use rgpui::line_break::{break_class, can_break_between};
    use rgpui::{
        App, Bounds, Context, IntoElement, ParentElement, Pixels, Render, SharedString,
        Styled as _, TextAlign, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct DirectionSample {
        /// 样本文本。
        text: &'static str,
        /// 样例说明。
        note: &'static str,
    }

    const DIRECTION_SAMPLES: &[DirectionSample] = &[
        DirectionSample {
            text: "你好，世界",
            note: "拉丁文本",
        },
        DirectionSample {
            text: "Привет, мир",
            note: "西里尔文本",
        },
        DirectionSample {
            text: "你好，世界",
            note: "中文（LTR）",
        },
        DirectionSample {
            text: "שלום עולם",
            note: "希伯来文",
        },
        DirectionSample {
            text: "مرحبا بالعالم",
            note: "阿拉伯文",
        },
        DirectionSample {
            text: "«שלום»",
            note: "首个强方向字符在标点之后",
        },
        DirectionSample {
            text: "12 שלום",
            note: "数字是弱方向字符，会被跳过",
        },
        DirectionSample {
            text: "٣٤٥ 12.5%",
            note: "纯数字 / 百分号：回退为 LTR",
        },
    ];

    struct PairSample {
        /// 前一个字符。
        prev: char,
        /// 后一个字符。
        next: char,
        /// 判定说明。
        note: &'static str,
    }

    const PAIR_SAMPLES: &[PairSample] = &[
        PairSample {
            prev: '你',
            next: '好',
            note: "汉字之间可断行",
        },
        PairSample {
            prev: '好',
            next: '。',
            note: "行首禁则：句号不可起行",
        },
        PairSample {
            prev: '话',
            next: '」',
            note: "行首禁则：闭引号不可起行",
        },
        PairSample {
            prev: '「',
            next: '你',
            note: "行末禁则：开引号不可收行",
        },
        PairSample {
            prev: 'あ',
            next: 'ぁ',
            note: "行首禁则：小写假名不可起行",
        },
        PairSample {
            prev: 'カ',
            next: 'ー',
            note: "行首禁则：长音符不可起行",
        },
        PairSample {
            prev: 'a',
            next: '\u{00A0}',
            note: "胶连字符：不换行空格两侧不可断",
        },
        PairSample {
            prev: 'a',
            next: ')',
            note: "半角标点仍按拉丁规则处理",
        },
    ];

    const CLASS_SAMPLES: &[char] = &['汉', 'あ', 'ぁ', 'ー', '。', '「', '한', 'a', ' ', '+'];

    const CJK_SAMPLE: &str = "他说：「今天的天气很好，我们一起去公园散步吧。」（真的很适合散步。）";

    struct IntlTextDemo;

    impl IntlTextDemo {
        /// 创建示例视图。
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self
        }
    }

    fn section(
        title: impl Into<SharedString>,
        body: impl IntoElement,
        cx: &App,
    ) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded(px(10.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .child(div().text_sm().child(title.into()))
            .child(body)
    }

    fn direction_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let rows = DIRECTION_SAMPLES.iter().map(|sample| {
            let direction = TextDirection::resolve(sample.text);
            let strong = sample
                .text
                .chars()
                .find_map(|c| strong_direction(c).map(|d| (c, d)));
            let strong_label = match strong {
                Some((c, TextDirection::Ltr)) => format!("首个强方向字符 {c:?} → LTR"),
                Some((c, TextDirection::Rtl)) => format!("首个强方向字符 {c:?} → RTL"),
                None => "没有强方向字符".to_string(),
            };

            div()
                .v_flex()
                .gap_1()
                .child(
                    div()
                        .w(px(260.0))
                        .px_3()
                        .py_2()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.background)
                        // 关键点：`Start` 让文本自动靠向段落的起始边。
                        .text_align(TextAlign::Start)
                        .child(sample.text),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{} · 解析方向 {} · {} · 含 RTL 字符：{}",
                            sample.note,
                            if direction.is_rtl() { "RTL" } else { "LTR" },
                            strong_label,
                            if sample.text.chars().any(is_rtl_char) {
                                "是"
                            } else {
                                "否"
                            },
                        )),
                )
        });

        section(
            "段落基准方向（HTML dir=auto 口径）—— 每个样例都用 text_align(Start) 排版",
            div().v_flex().gap_3().children(rows),
            cx,
        )
    }

    fn align_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let text = "שלום עולם";
        let alphas = [
            (TextAlign::Left, "Left", "显式左对齐（与文字方向无关）"),
            (
                TextAlign::Start,
                "开始",
                "起始边：RTL 段落为右 —— 段落方向说话",
            ),
            (TextAlign::End, "End", "结束边：RTL 段落为左"),
            (TextAlign::Right, "Right", "显式右对齐"),
        ];

        let rows = alphas.into_iter().map(|(align, name, note)| {
            div().v_flex().gap_1().child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(200.0))
                            .px_3()
                            .py_2()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.background)
                            .text_align(align)
                            .child(text),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{name} —— {note}")),
                    ),
            )
        });

        section(
            "start / end 随段落方向翻转",
            div().v_flex().gap_2().children(rows),
            cx,
        )
    }

    fn kinsoku_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();

        let narrow = |width: Pixels, cx: &App| {
            div()
                .w(width)
                .px_2()
                .py_2()
                .rounded(px(6.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .text_sm()
                .child(CJK_SAMPLE)
        };

        let pairs = PAIR_SAMPLES.iter().map(|sample| {
            let allowed = can_break_between(sample.prev, sample.next);
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(120.0))
                        .font_family("monospace")
                        .child(format!("{} + {}", sample.prev, sample.next)),
                )
                .child(
                    div()
                        .w(px(48.0))
                        .text_color(if allowed { theme.success } else { theme.danger })
                        .child(if allowed { "可断 ✓" } else { "禁则 ✗" }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(sample.note),
                )
        });

        section(
            "东亚禁则（kinsoku）：窄容器里换行点不会落在行首 / 行末禁则字符上",
            div()
                .v_flex()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .items_start()
                        .child(
                            div()
                                .v_flex()
                                .gap_1()
                                .child(div().text_xs().child("宽 160px"))
                                .child(narrow(px(160.0), cx)),
                        )
                        .child(
                            div()
                                .v_flex()
                                .gap_1()
                                .child(div().text_xs().child("宽 110px"))
                                .child(narrow(px(110.0), cx)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "样例：{CJK_SAMPLE} —— 拉窄看换行点：句号不在行首、开引号不在行末。"
                        )),
                )
                .child(div().h(px(1.0)).bg(theme.border))
                .child(div().v_flex().gap_1().children(pairs)),
            cx,
        )
    }

    fn class_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let rows = CLASS_SAMPLES.iter().map(|c| {
            let class = break_class(*c);
            let name = match class {
                rgpui::line_break::BreakClass::Ideographic => "汉字（可断）",
                rgpui::line_break::BreakClass::Kana => "假名（可断）",
                rgpui::line_break::BreakClass::Hangul => "谚文（可断）",
                rgpui::line_break::BreakClass::SmallKana => "小写假名（行首禁则）",
                rgpui::line_break::BreakClass::ProlongedSoundMark => "长音符（行首禁则）",
                rgpui::line_break::BreakClass::ClosePunctuation => "闭标点（行首禁则）",
                rgpui::line_break::BreakClass::OpenPunctuation => "开标点（行末禁则）",
                rgpui::line_break::BreakClass::Word => "单词字符",
                rgpui::line_break::BreakClass::Space => "空白",
                rgpui::line_break::BreakClass::Other => "其他",
            };
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .w(px(60.0))
                        .font_family("monospace")
                        .child(c.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{class:?} · {name}")),
                )
        });

        section(
            "break_class：断行规则眼中的字符类别",
            div().v_flex().gap_1().children(rows),
            cx,
        )
    }

    fn i18n_section(cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let locales = [
            "zh-CN", "en-US", "ja-JP", "ar-EG", "he-IL", "fa-IR", "ur-PK",
        ];
        let rows = locales.into_iter().map(|locale| {
            let direction = TextDirection::for_locale(locale);
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(div().w(px(80.0)).font_family("monospace").child(locale))
                .child(
                    div()
                        .w(px(48.0))
                        .text_xs()
                        .text_color(if direction.is_rtl() {
                            theme.danger
                        } else {
                            theme.success
                        })
                        .child(if direction.is_rtl() { "RTL" } else { "LTR" }),
                )
                .child(div().text_xs().text_color(theme.muted_foreground).child(
                    if direction.is_rtl() {
                        "外壳应镜像：滚动条在左、图标方向翻转"
                    } else {
                        "按 LTR 排版"
                    },
                ))
        });

        // 故意让 en 少一个 key，`validate` 会把它列出来。
        let mut i18n = I18nManager::new("zh-CN").with_fallback("en");
        i18n.load_translations_map(
            "zh-CN",
            HashMap::from([
                ("hello".to_string(), "你好".to_string()),
                ("bye".to_string(), "再见".to_string()),
            ]),
        );
        i18n.load_translations_map(
            "en",
            HashMap::from([("hello".to_string(), "Hello".to_string())]),
        );
        let problems = i18n.validate();

        section(
            "语言标签 → 默认方向；语言包审计（en 故意缺一个 key）",
            div()
                .v_flex()
                .gap_3()
                .child(div().v_flex().gap_1().children(rows))
                .child(div().h(px(1.0)).bg(theme.border))
                .child(
                    div().v_flex().gap_1().children(
                        problems
                            .into_iter()
                            .map(|problem| div().text_xs().text_color(theme.danger).child(problem)),
                    ),
                ),
            cx,
        )
    }

    impl Render for IntlTextDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = cx.theme().clone();

            div()
                .v_flex()
                .size_full()
                .bg(theme.background)
                .text_color(theme.foreground)
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().text_lg().child("国际文本排版（bidi + 东亚断行）"))
                        .child(div().text_xs().text_color(theme.muted_foreground).child(
                            "方向判定取首个强方向字符；Start / End 随段落方向翻转；\
                                 换行遵守 CJK 行首 / 行末禁则与胶连字符。",
                        )),
                )
                .child(
                    div()
                        .id("intl-text-scroll")
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .p_4()
                        .v_flex()
                        .gap_4()
                        .child(direction_section(cx))
                        .child(align_section(cx))
                        .child(kinsoku_section(cx))
                        .child(class_section(cx))
                        .child(i18n_section(cx)),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(980.0), px(760.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("国际文本示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| IntlTextDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| IntlTextDemo::new(window, cx)).into()
    }
}

mod markdown {
    //! Markdown 渲染示例。
    //!
    //! 演示两条 Markdown 相关能力：
    //! - **块级渲染**：`BlockRenderer::parse_markdown` 把 Markdown 拆成 `BlockElement`
    //!   （标题/段落/代码块/引用/分隔线…），再由调用方按 `BlockType` 自行渲染。
    //! - **插件式渲染**：`MarkdownRenderer` 注册内置插件（代码高亮/数学/任务列表）后
    //!   输出渲染结果字符串，并可自定义样式。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin markdown
    //! ```

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::markdown_ext::{CodeHighlightPlugin, MathPlugin, TaskListPlugin};
    use rgpui::{
        AnyElement, App, Bounds, Context, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    struct MarkdownDemo {
        /// 待解析的 Markdown 文本。
        input: String,
        /// 插件式渲染器。
        renderer: MarkdownRenderer,
    }

    impl MarkdownDemo {
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            // 注册内置插件：代码高亮、数学、任务列表。
            let mut renderer = MarkdownRenderer::new();
            renderer.register_plugin(Box::new(CodeHighlightPlugin));
            renderer.register_plugin(Box::new(MathPlugin));
            renderer.register_plugin(Box::new(TaskListPlugin));
            renderer.add_style("highlight", "background: yellow;");

            Self {
            input: "# 标题\n\n这是 **粗体** 和 *斜体* 段落。\n\n> 一段引用\n\n```rust\nfn main() {}\n```\n\n---\n"
                .to_string(),
            renderer,
        }
        }
    }

    impl Render for MarkdownDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let renderer = BlockRenderer::new();
            let blocks = renderer.parse_markdown(&self.input);
            let plugin_output = self.renderer.render(&self.input);

            v_flex()
                .id("markdown-demo")
                .size_full()
                .gap(px(16.0))
                .p(px(24.0))
                .overflow_scroll()
                .bg(rgb(0xffffff))
                .child(div().text_2xl().child("Markdown 渲染"))
                // 1. 块级渲染。
                .child(section_title("1. 块级渲染（BlockRenderer）"))
                .child(
                    v_flex()
                        .gap(px(6.0))
                        .p(px(12.0))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0xe0e0e0))
                        .children(blocks.into_iter().map(render_block_element)),
                )
                // 2. 插件式渲染。
                .child(section_title("2. 插件式渲染（MarkdownRenderer）"))
                .child(
                    div()
                        .rounded_md()
                        .bg(rgb(0x282c34))
                        .p(px(12.0))
                        .text_sm()
                        .text_color(rgb(0xabb2bf))
                        .child(plugin_output),
                )
        }
    }

    fn section_title(text: &str) -> impl IntoElement {
        div().text_lg().child(text.to_string())
    }

    fn render_block_element(block: BlockElement) -> AnyElement {
        match &block.block_type {
            BlockType::Heading(level) => {
                let base = div()
                    .id(format!("md-h-{}", block.content))
                    .mb(px(4.0))
                    .text_color(rgb(0x1a1a1a));
                let styled = match level {
                    1 => base.text_2xl().font_bold(),
                    2 => base.text_xl().font_semibold(),
                    3 => base.text_lg().font_medium(),
                    _ => base.text_base(),
                };
                styled.child(block.content.clone()).into_any_element()
            }
            BlockType::Paragraph => div()
                .id(format!(
                    "md-p-{}",
                    block.content.chars().take(8).collect::<String>()
                ))
                .mb(px(8.0))
                .text_sm()
                .text_color(rgb(0x333333))
                .child(block.content.clone())
                .into_any_element(),
            BlockType::CodeBlock => {
                let lang = block
                    .attributes
                    .get("language")
                    .map(|s| s.as_str())
                    .unwrap_or("");
                div()
                    .id("md-code")
                    .mb(px(8.0))
                    .rounded_md()
                    .bg(rgb(0x282c34))
                    .p(px(12.0))
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x888888))
                                    .child(format!("语言: {lang}")),
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
                .id("md-quote")
                .mb(px(8.0))
                .pl(px(12.0))
                .border_l(px(3.0))
                .border_color(rgb(0x0078d4))
                .text_sm()
                .text_color(rgb(0x666666))
                .child(block.content.clone())
                .into_any_element(),
            BlockType::HorizontalRule => div()
                .id("md-hr")
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

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(720.0), px(640.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| MarkdownDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| MarkdownDemo::new(window, cx)).into()
    }
}

mod markdown_stream {
    //! Markdown 流式增量渲染示例。
    //!
    //! 演示 `MarkdownState` + `Markdown::new_state`：
    //! - 源码按小块逐步追加，模拟大模型逐 token 输出；
    //! - 每次追加后防抖 16ms 在后台整篇重解析，解析期间继续渲染上一版块列表，
    //!   因此流式输出时主线程不会每来一个 chunk 就重建整篇文档。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin markdown_stream
    //! ```

    use std::time::Duration;

    use rgpui::prelude::*;
    use rgpui::{
        App, Bounds, Button, Context, Entity, IntoElement, Render, Task, Window, WindowBounds,
        WindowOptions, div, h_flex, px, rgb, size, v_flex,
    };
    use rgpui_markdown::{Markdown, MarkdownState};
    use rgpui_platform::application;

    const SCRIPT: &str = r##"# 流式 Markdown

这条回答由一个字符一个字符地“生成”，但**只有解析完成后才整体替换**上一版内容。

## 能力清单

- 标题、段落与**粗体** / *斜体*
- 有序与无序列表
- 行内代码 `MarkdownState::append`
- 围栏代码块

```rust
let state = cx.new(|_| MarkdownState::new(""));
state.update(cx, |state, cx| state.append("# 你好", cx));
```

> 追加调用只更新源码并入队一次防抖解析；
> 渲染取的是最近一次成功的解析结果。

| 字段 | 含义 |
| --- | --- |
| `source` | 已接收的完整源码 |
| `parsing` | 是否有后台解析未完成 |

最后一段：滚动查看上方内容，解析状态行会在完成后变为“已就绪”。
"##;

    struct StreamingDemo {
        /// 持有源码与最近一次解析结果的实体。
        state: Entity<MarkdownState>,
        /// 已注入的字节数（用于进度展示）。
        received: usize,
        /// 整篇源码长度。
        total: usize,
        /// 是否已输出完毕。
        done: bool,
        /// 当前输出任务；重新播放时丢弃即取消旧任务。
        stream_task: Option<Task<()>>,
        /// 右上角帧率/绘制耗时浮层，用于观察流式输出期间的开销。
        hud: Entity<FpsHud>,
    }

    impl StreamingDemo {
        /// 创建视图并立即开始输出。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let state = cx.new(|_| MarkdownState::new(""));
            let hud_state = cx.new(|_| FpsHudState::new());
            let hud = cx.new(|_| FpsHud::new(hud_state).continuous(true));
            let mut demo = Self {
                state,
                received: 0,
                total: SCRIPT.len(),
                done: false,
                stream_task: None,
                hud,
            };
            demo.start(cx);
            demo
        }

        /// 清空并重新开始逐块输出。
        fn start(&mut self, cx: &mut Context<Self>) {
            self.received = 0;
            self.done = false;
            self.state.update(cx, |state, cx| state.set_source("", cx));

            let chunks = chunk_source(SCRIPT, 32);
            self.stream_task = Some(cx.spawn(async move |this, cx| {
                for chunk in chunks {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let alive = this.update(cx, |demo, cx| {
                        demo.received += chunk.len();
                        demo.state.update(cx, |state, cx| state.append(chunk, cx));
                        cx.notify();
                    });
                    if alive.is_err() {
                        return;
                    }
                }
                _ = this.update(cx, |demo, cx| {
                    demo.done = true;
                    cx.notify();
                });
            }));
        }
    }

    impl Render for StreamingDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let status = if self.done {
                format!("已就绪：{} 字节", self.total)
            } else {
                format!("输出中：{}/{} 字节", self.received, self.total)
            };

            v_flex()
                .relative()
                .size_full()
                .gap(px(12.0))
                .p(px(24.0))
                .bg(rgb(0xffffff))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_xl().child("Markdown 流式增量渲染"))
                        .child(
                            Button::new("markdown-stream-restart")
                                .label("重新播放")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.start(cx);
                                })),
                        ),
                )
                .child(div().text_sm().text_color(rgb(0x666666)).child(status))
                .child(
                    v_flex()
                        .id("markdown-stream-scroll")
                        .flex_1()
                        .w_full()
                        .min_h(px(0.0))
                        .overflow_scroll()
                        .child(Markdown::new_state(self.state.clone()).w_full()),
                )
                .child(self.hud.clone())
        }
    }

    fn chunk_source(source: &'static str, chunk_bytes: usize) -> Vec<&'static str> {
        let mut chunks = Vec::new();
        let mut rest = source;
        while !rest.is_empty() {
            let mut end = chunk_bytes.min(rest.len());
            while !rest.is_char_boundary(end) {
                end -= 1;
            }
            let (chunk, tail) = rest.split_at(end);
            chunks.push(chunk);
            rest = tail;
        }
        chunks
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(760.0), px(680.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| StreamingDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| StreamingDemo::new(window, cx)).into()
    }
}

mod markdown_selection {
    //! Markdown 文档级选区示例。
    //!
    //! 演示 `MarkdownSelectionState` + `Markdown::selection`：
    //! - 在正文内按住左键拖选，跨段落 / 列表 / 表格单元格连续选中；
    //! - `Ctrl+C` 复制选中文本（列表项保留 `• ` / `1. ` 标记）；
    //! - `Ctrl+A` 全选、`Esc` 清空选区。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin markdown_selection
    //! ```

    use rgpui::prelude::*;
    use rgpui::{
        App, Bounds, Button, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, MouseButton,
        Render, Subscription, Window, WindowBounds, WindowOptions, div, h_flex, px, rgb, size,
        v_flex,
    };
    use rgpui_markdown::{Markdown, MarkdownSelectionState};
    use rgpui_platform::application;

    const DOC: &str = r##"# 文档级选区

在任意段落里按住左键拖动即可跨段落选择；**加粗**、*斜体*、`行内代码` 与
[链接](https://example.com) 都在同一段文本叶子里。

## 列表

- 第一项：列表标记会随文本一起复制
- 第二项：拖动时经过的整段会被选中
- 第三项：嵌套列表同样可选

1. 有序列表第一项
2. 有序列表第二项

> 引用块里的段落也是普通文本叶子，
> 跨行拖选会按阅读顺序拼接。

## 代码块

```rust
fn main() {
    println!("代码块按整块选中");
}
```

| 区域 | 选区行为 |
| --- | --- |
| 段落 | 按字符命中 |
| 表格单元格 | 按单元格文本命中 |
| 代码块 | 整块选中 |

最后一段用于验证拖选能一直延伸到文末。
"##;

    struct SelectionDemo {
        /// 选区状态实体。
        selection: Entity<MarkdownSelectionState>,
        /// 键盘焦点（`Ctrl+C` / `Ctrl+A` / `Esc`）。
        focus: FocusHandle,
        /// 观察选区实体变化，驱动重绘。
        _selection_subscription: Subscription,
        /// 最近一次操作提示。
        notice: Option<String>,
    }

    impl SelectionDemo {
        /// 创建视图。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let selection = cx.new(|_| MarkdownSelectionState::new());
            let subscription = cx.observe(&selection, |_, _, cx| cx.notify());
            Self {
                selection,
                focus: cx.focus_handle(),
                _selection_subscription: subscription,
                notice: None,
            }
        }

        /// 复制当前选区到系统剪贴板。
        fn copy(&mut self, cx: &mut Context<Self>) {
            let Some(text) = self.selection.read(cx).selected_text() else {
                self.notice = Some("没有选中文本".to_string());
                cx.notify();
                return;
            };
            let chars = text.chars().count();
            let copied = self
                .selection
                .update(cx, |state, cx| state.copy_to_clipboard(cx));
            self.notice = Some(if copied {
                format!("已复制 {chars} 个字符")
            } else {
                "复制失败".to_string()
            });
            cx.notify();
        }
    }

    impl Render for SelectionDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let selected = self.selection.read(cx).selected_text();
            let status = match (&self.notice, &selected) {
                (Some(notice), _) => notice.clone(),
                (None, Some(text)) => format!("已选中 {} 个字符", text.chars().count()),
                (None, None) => "拖选正文，Ctrl+C 复制，Ctrl+A 全选".to_string(),
            };

            v_flex()
                .track_focus(&self.focus)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _event, window, cx| {
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }),
                )
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                    if !event.keystroke.modifiers.secondary() {
                        return;
                    }
                    match event.keystroke.key.to_ascii_lowercase().as_str() {
                        "c" => this.copy(cx),
                        "a" => {
                            this.selection.update(cx, |state, cx| state.select_all(cx));
                            this.notice = Some("已全选".to_string());
                            cx.notify();
                        }
                        _ => {}
                    }
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                    if event.keystroke.key == "escape" {
                        this.selection.update(cx, |state, cx| state.clear(cx));
                        this.notice = None;
                        cx.notify();
                    }
                }))
                .relative()
                .size_full()
                .gap(px(12.0))
                .p(px(24.0))
                .bg(rgb(0xffffff))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_xl().child("Markdown 文档级选区"))
                        .child(
                            Button::new("markdown-selection-clear")
                                .label("清空选区")
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.selection.update(cx, |state, cx| state.clear(cx));
                                    this.notice = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(div().text_sm().text_color(rgb(0x555555)).child(status))
                .child(
                    v_flex()
                        .id("markdown-selection-scroll")
                        .flex_1()
                        .w_full()
                        .min_h(px(0.0))
                        .overflow_scroll()
                        .child(
                            Markdown::new(DOC)
                                .selection(self.selection.clone())
                                .w_full(),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(760.0), px(680.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| SelectionDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }
}

mod rich_text {
    //! 富文本示例
    //!
    //! 演示 `rich_text` 文档模型（段落 + 行内运行）与只读渲染元素 `RichText`：
    //! 在字符坐标上对选区应用粗体/斜体/下划线/删除线/颜色，切换段落对齐与列表，
    //! 并演示纯文本 / HTML 双向转换与富剪贴板对接（复制为 HTML，粘贴时反解析）。
    //!
    //! 运行：`cargo run -p showcase --bin rich_text`

    use std::ops::Range;

    use rgpui::rich_text::{
        ListKind, Paragraph, RichText, RichTextDoc, RichTextHistory, Run, RunProps,
    };
    use rgpui::{
        App, Bounds, Button, ClipboardItem, Context, Entity, IntoElement, ParentElement, Render,
        SharedString, Styled as _, TextAlign, TitlebarOptions, Window, WindowBounds, WindowOptions,
        div, hsla, prelude::*, px, size,
    };
    use rgpui_platform::application;

    type StyleApply = fn(&mut RunProps);

    const STYLE_TARGETS: &[(&str, StyleApply, &str)] = &[
        ("加粗", |p| p.bold = true, "加粗"),
        ("斜体", |p| p.italic = true, "斜体"),
        ("下划线", |p| p.underline = true, "下划线"),
        ("删除线", |p| p.strikethrough = true, "删除线"),
    ];

    struct RichTextApp {
        /// 指向自身的句柄，供按钮回调更新状态。
        handle: Entity<Self>,
        /// 当前文档。
        doc: RichTextDoc,
        /// 撤销 / 重做历史。
        history: RichTextHistory,
        /// 最近一次操作的结果提示。
        status: SharedString,
        /// 查找命中的字符区间（渲染为搜索高亮）。
        highlight: Vec<std::ops::Range<usize>>,
    }

    impl RichTextApp {
        /// 创建示例应用（含一段带样式的样例文档）。
        fn new(cx: &mut Context<Self>) -> Self {
            let doc = sample_doc();
            Self {
                handle: cx.entity(),
                history: RichTextHistory::new(),
                doc,
                status: "点击按钮修改文档；右侧/下方可查看 HTML 序列化结果".into(),
                highlight: Vec::new(),
            }
        }

        /// 对当前文档的纯文本范围应用一次行内样式。
        fn style_word(&mut self, label: &str, f: impl Fn(&mut RunProps), cx: &mut Context<Self>) {
            let plain = self.doc.plain_text();
            match char_range(&plain, label) {
                Some(range) => {
                    self.history.edit(&mut self.doc, |doc| {
                        doc.apply_props(range, f);
                    });
                    self.status = format!("已对「{label}」应用样式").into();
                }
                None => self.status = format!("未在文档中找到「{label}」").into(),
            }
            cx.notify();
        }

        /// 设置首段对齐。
        fn align_title(&mut self, align: TextAlign, cx: &mut Context<Self>) {
            if let Some(para) = self.doc.paragraphs.first_mut() {
                para.style.align = align;
            }
            self.status = format!("标题对齐 → {align:?}").into();
            cx.notify();
        }

        /// 切换第二段（索引 1）的项目符号。
        fn toggle_bullet(&mut self, cx: &mut Context<Self>) {
            if let Some(para) = self.doc.paragraphs.get_mut(1) {
                para.style.list = if para.style.list == ListKind::Bullet {
                    ListKind::None
                } else {
                    ListKind::Bullet
                };
                self.status = format!("第二段列表 → {:?}", para.style.list).into();
            }
            cx.notify();
        }

        /// 清除全文字内样式。
        fn clear_inline(&mut self, cx: &mut Context<Self>) {
            let total = self.doc.char_len();
            self.doc
                .apply_props(0..total, |props| *props = RunProps::new());
            self.status = "已清除全文字内样式".into();
            cx.notify();
        }

        /// 把文档以「HTML + 纯文本」写入剪贴板。
        fn copy_html(&mut self, cx: &mut Context<Self>) {
            let html = self.doc.to_html();
            let plain = self.doc.plain_text();
            cx.write_to_clipboard(ClipboardItem::new_html(html, Some(plain)));
            self.status = "已复制为 HTML + 纯文本".into();
            cx.notify();
        }

        /// 从剪贴板读回并替换文档（优先 HTML，退回纯文本）。
        fn paste(&mut self, cx: &mut Context<Self>) {
            let Some(item) = cx.read_from_clipboard() else {
                self.status = "剪贴板为空或格式不受支持".into();
                cx.notify();
                return;
            };
            if let Some(html) = item.html()
                && let Some(doc) = RichTextDoc::from_html(html)
            {
                self.status = format!("已从 HTML 粘贴（{} 段）", doc.paragraphs.len()).into();
                self.doc = doc;
                cx.notify();
                return;
            }
            if let Some(text) = item.text() {
                self.doc = RichTextDoc::from_plain_text(&text);
                self.status = "剪贴板无 HTML，已按纯文本粘贴".into();
            } else {
                self.status = "剪贴板没有可用文本".into();
            }
            cx.notify();
        }

        /// 恢复样例文档。
        fn reset(&mut self, cx: &mut Context<Self>) {
            self.doc = sample_doc();
            self.status = "已重置为样例文档".into();
            cx.notify();
        }

        /// 给正文里的某个词加一条批注。
        fn add_annotation(&mut self, cx: &mut Context<Self>) {
            let plain = self.doc.plain_text();
            match char_range(&plain, "颜色") {
                Some(range) => {
                    let id = self.doc.add_annotation(range, "批注示例：这里用红色标注");
                    if let Some(annotation) = self.doc.annotation_mut(id) {
                        annotation.author = Some("示例".to_string());
                    }
                    self.status = format!("已添加批注 #{id}").into();
                }
                None => self.status = "未在文档中找到「颜色」".into(),
            }
            cx.notify();
        }

        /// 删除第一条批注。
        fn remove_first_annotation(&mut self, cx: &mut Context<Self>) {
            match self.doc.annotations.first().map(|item| item.id) {
                Some(id) => {
                    self.doc.remove_annotation(id);
                    self.status = format!("已删除批注 #{id}").into();
                }
                None => self.status = "当前没有批注".into(),
            }
            cx.notify();
        }

        /// 在正文末尾插入脚注引用。
        fn add_footnote(&mut self, cx: &mut Context<Self>) {
            let len = self.doc.char_len();
            let id = self.doc.insert_footnote(len, "*", "脚注示例：文末脚注内容");
            self.status = format!("已插入脚注 #{id}").into();
            cx.notify();
        }

        /// 删除第一条脚注（引用标记一并移除）。
        fn remove_first_footnote(&mut self, cx: &mut Context<Self>) {
            match self.doc.footnotes.first().map(|item| item.id) {
                Some(id) => {
                    self.doc.remove_footnote(id);
                    self.status = format!("已删除脚注 #{id}").into();
                }
                None => self.status = "当前没有脚注".into(),
            }
            cx.notify();
        }

        /// 查找并替换全部命中。
        fn replace_all(&mut self, needle: &str, with: &str, cx: &mut Context<Self>) {
            let hits = self.doc.find(needle, true);
            if hits.is_empty() {
                self.status = format!("未找到「{needle}」").into();
                cx.notify();
                return;
            }
            let mut count = 0usize;
            self.history.edit(&mut self.doc, |doc| {
                count = doc.replace_all(needle, with, true);
            });
            self.status = format!("把「{needle}」替换为「{with}」：命中 {count} 处").into();
            cx.notify();
        }

        /// 查找「加粗」并高亮命中区间。
        fn find_and_highlight(&mut self, cx: &mut Context<Self>) {
            self.highlight = self.doc.find("加粗", true);
            self.status = format!("命中 {} 处「加粗」", self.highlight.len()).into();
            cx.notify();
        }

        /// 清除查找高亮。
        fn clear_highlight(&mut self, cx: &mut Context<Self>) {
            self.highlight.clear();
            self.status = "已清除查找高亮".into();
            cx.notify();
        }

        /// 撤销一步。
        fn undo(&mut self, cx: &mut Context<Self>) {
            if self.history.undo(&mut self.doc) {
                self.status = "已撤销".into();
            } else {
                self.status = "没有可撤销的操作".into();
            }
            cx.notify();
        }

        /// 重做一步。
        fn redo(&mut self, cx: &mut Context<Self>) {
            if self.history.redo(&mut self.doc) {
                self.status = "已重做".into();
            } else {
                self.status = "没有可重做的操作".into();
            }
            cx.notify();
        }
    }

    impl Render for RichTextApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let handle = self.handle.clone();
            let mut buttons = div().flex().flex_row().flex_wrap().gap_2();

            // 行内样式按钮：命中样例文档中的固定词。
            for (needle, apply, label) in STYLE_TARGETS {
                let handle = handle.clone();
                let needle = needle.to_string();
                let title = label.to_string();
                buttons = buttons.child(
                    Button::new(format!("style-{needle}"))
                        .label(title)
                        .on_click(move |_event, _window, cx| {
                            handle.update(cx, |this, cx| this.style_word(&needle, *apply, cx));
                        }),
                );
            }

            // 颜色按钮（单独处理，因为要传具体色值）。
            buttons = buttons.child(Button::new("style-color").label("红色").on_click({
                let handle = handle.clone();
                move |_event, _window, cx| {
                    handle.update(cx, |this, cx| {
                        this.style_word("颜色", |p| p.color = Some(hsla(0.0, 0.85, 0.5, 1.0)), cx);
                    });
                }
            }));

            buttons = buttons
                .child(Button::new("clear-inline").label("清除字内样式").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.clear_inline(cx))
                }))
                .child(Button::new("align-center").label("标题居中").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| {
                        handle.update(cx, |this, cx| this.align_title(TextAlign::Center, cx));
                    }
                }))
                .child(Button::new("align-left").label("标题左对齐").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| {
                        handle.update(cx, |this, cx| this.align_title(TextAlign::Left, cx));
                    }
                }))
                .child(Button::new("bullet").label("切换第二段列表").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.toggle_bullet(cx))
                }))
                .child(Button::new("copy-html").label("复制为 HTML").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.copy_html(cx))
                }))
                .child(Button::new("add-annotation").label("添加批注").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.add_annotation(cx))
                }))
                .child(
                    Button::new("remove-annotation")
                        .label("删除首条批注")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle.update(cx, |this, cx| this.remove_first_annotation(cx))
                            }
                        }),
                )
                .child(Button::new("add-footnote").label("插入脚注").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.add_footnote(cx))
                }))
                .child(
                    Button::new("remove-footnote")
                        .label("删除首条脚注")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle.update(cx, |this, cx| this.remove_first_footnote(cx))
                            }
                        }),
                )
                .child(
                    Button::new("find-add")
                        .label("查找并高亮「加粗」")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle.update(cx, |this, cx| this.find_and_highlight(cx))
                            }
                        }),
                )
                .child(Button::new("find-clear").label("清除高亮").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| {
                        handle.update(cx, |this, cx| this.clear_highlight(cx))
                    }
                }))
                .child(
                    Button::new("replace-all")
                        .label("替换「项目符号」→「条目」")
                        .on_click({
                            let handle = handle.clone();
                            move |_event, _window, cx| {
                                handle
                                    .update(cx, |this, cx| this.replace_all("项目符号", "条目", cx))
                            }
                        }),
                )
                .child(Button::new("undo").label("撤销").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.undo(cx))
                }))
                .child(Button::new("redo").label("重做").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.redo(cx))
                }))
                .child(Button::new("paste").label("从剪贴板粘贴").on_click({
                    let handle = handle.clone();
                    move |_event, _window, cx| handle.update(cx, |this, cx| this.paste(cx))
                }))
                .child(
                    Button::new("reset")
                        .label("重置")
                        .on_click(move |_event, _window, cx| {
                            handle.update(cx, |this, cx| this.reset(cx))
                        }),
                );

            let html = self.doc.to_html();
            let html_preview = preview(&html, 320);
            // 批注点击：回传到应用状态（示例展示批注 id）。
            let annotation_handle = self.handle.clone();
            // 链接点击：回传到应用状态（示例展示链接地址）。
            let link_handle = self.handle.clone();

            // 批注 / 脚注清单（展示模型内容与区间）。
            let mut notes = div().flex().flex_col().gap_1().text_xs();
            for annotation in &self.doc.annotations {
                notes = notes.child(format!(
                    "批注 #{} [{}..{}] {}：{}",
                    annotation.id,
                    annotation.range.start,
                    annotation.range.end,
                    annotation.author.as_deref().unwrap_or("匿名"),
                    annotation.note
                ));
            }
            for footnote in &self.doc.footnotes {
                notes = notes.child(format!(
                    "脚注 #{} [{}..{}]：{}",
                    footnote.id, footnote.range.start, footnote.range.end, footnote.text
                ));
            }
            if self.doc.annotations.is_empty() && self.doc.footnotes.is_empty() {
                notes = notes.child("（暂无批注与脚注）");
            }

            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(div().text_xl().child("富文本示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(buttons)
                .child(
                    div()
                        .flex_1()
                        .min_h(px(160.0))
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .child(
                            RichText::new(self.doc.clone())
                                .highlight_ranges(self.highlight.clone())
                                .on_annotation_click(move |id, _window, cx| {
                                    annotation_handle.update(cx, |this, cx| {
                                        this.status = format!("点击了批注 #{id}").into();
                                        cx.notify();
                                    });
                                })
                                .on_link_click(move |href, _window, cx| {
                                    link_handle.update(cx, |this, cx| {
                                        this.status = format!("点击了链接：{href}").into();
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("批注与脚注："),
                )
                .child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_xs()
                        .child(notes),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("HTML 序列化："),
                )
                .child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_xs()
                        .child(html_preview),
                )
        }
    }

    fn sample_doc() -> RichTextDoc {
        let mut doc = RichTextDoc::new();

        let mut title = Paragraph::default();
        title.style.align = TextAlign::Center;
        title.style.heading = Some(1);
        title.runs = vec![Run::with("rgpui 富文本原型", RunProps::new().bold())];

        let body = Paragraph {
            style: Default::default(),
            runs: vec![
                Run::new("支持 "),
                Run::with("加粗", RunProps::new().bold()),
                Run::new("、"),
                Run::with("斜体", RunProps::new().italic()),
                Run::new("、"),
                Run::with("下划线", RunProps::new().underline()),
                Run::new("、"),
                Run::with("删除线", RunProps::new().strikethrough()),
                Run::new("，以及 "),
                Run::with("颜色", RunProps::new().color(hsla(0.0, 0.85, 0.5, 1.0))),
                Run::new("，还可以加 "),
                Run::with_link(
                    "链接",
                    RunProps::default(),
                    "https://github.com/launcher-rs/rgpui",
                ),
                Run::new("。"),
            ],
        };

        let mut bullet_a = Paragraph::new("项目符号一");
        bullet_a.style.list = ListKind::Bullet;
        let mut bullet_b = Paragraph::new("项目符号二");
        bullet_b.style.list = ListKind::Bullet;

        let mut number_a = Paragraph::new("编号一");
        number_a.style.list = ListKind::Number;
        let mut number_b = Paragraph::new("编号二");
        number_b.style.list = ListKind::Number;

        doc.paragraphs = vec![title, body, bullet_a, bullet_b, number_a, number_b];

        // 预置一条批注与一个脚注（正文高亮 + 文末脚注）。
        let plain = doc.plain_text();
        if let Some(range) = char_range(&plain, "加粗") {
            let id = doc.add_annotation(range, "批注示例：加粗处可再确认语气");
            if let Some(annotation) = doc.annotation_mut(id) {
                annotation.author = Some("示例".to_string());
            }
        }
        let len = doc.char_len();
        doc.insert_footnote(len, "*", "脚注示例：文末脚注内容");
        // 修订标记：插入为主色下划线，删除为删除线变暗。
        let revised = Paragraph::new("这一段演示修订：插入与删除。");
        doc.paragraphs.push(revised);
        doc.mark_inserted(0..4);
        doc.mark_deleted(10..13);

        // 分栏节（段落复用，按栏数排布）。
        doc.add_section(
            &[
                "左栏第一段。",
                "左栏第二段。",
                "右栏第一段。",
                "右栏第二段。",
            ],
            2,
            Some("分栏小节"),
        );

        doc
    }

    fn char_range(haystack: &str, needle: &str) -> Option<Range<usize>> {
        let start_byte = haystack.find(needle)?;
        let start = haystack[..start_byte].chars().count();
        let len = needle.chars().count();
        Some(start..start + len)
    }

    fn preview(text: &str, max: usize) -> String {
        if text.chars().count() <= max {
            return text.to_string();
        }
        let mut out: String = text.chars().take(max).collect();
        out.push('…');
        out
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(760.), px(620.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Rich Text Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(RichTextApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(RichTextApp::new).into()
    }
}

mod rich_clipboard {
    //! 富剪贴板示例
    //!
    //! 演示 `ClipboardItem` 的富负载能力（`ClipboardFormat` / `ClipboardEntry::Format`）：
    //! 复制时同时写入纯文本与 HTML（或 RTF）两种表示，粘贴时读回剪贴板上所有格式并逐条展示。
    //! 平台层负责把 MIME 映射到原生格式（Windows 注册格式 / `CF_HTML`、macOS UTType、
    //! Linux MIME atom），应用侧只管 MIME 与字节。
    //!
    //! 运行：`cargo run -p showcase --bin rich_clipboard`

    use rgpui::{
        App, Bounds, Button, ClipboardFormat, ClipboardItem, Context, Entity, IntoElement,
        ParentElement, Render, SharedString, Styled as _, TitlebarOptions, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    const SAMPLE_TEXT: &str = "rgpui 富剪贴板";
    const SAMPLE_HTML: &str = "<p><b>rgpui</b> <i>富剪贴板</i></p>";
    const SAMPLE_RTF: &str = r"{\rtf1\ansi rgpui 富剪贴板}";
    const SAMPLE_CSV: &str = "name,qty\napple,3\npear,5";

    struct RichClipboardApp {
        /// 指向自身的句柄，供按钮回调更新状态
        self_handle: Entity<Self>,
        /// 最近一次操作的结果提示
        status: SharedString,
        /// 读回剪贴板后各格式的展示文本
        formats: Vec<SharedString>,
    }

    impl RichClipboardApp {
        /// 创建示例应用
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                self_handle: cx.entity(),
                status: "点击「复制」写入富剪贴板，再点「粘贴」读回".into(),
                formats: Vec::new(),
            }
        }

        /// 把给定项目写入系统剪贴板并回显条目数
        fn copy(&mut self, item: ClipboardItem, label: &str, cx: &mut Context<Self>) {
            let count = item.entries().len();
            cx.write_to_clipboard(item);
            self.status = format!("已复制 {label}（{count} 个条目）").into();
            cx.notify();
        }

        /// 读取系统剪贴板，列出其中的每一种格式
        fn paste(&mut self, cx: &mut Context<Self>) {
            let Some(item) = cx.read_from_clipboard() else {
                self.status = "剪贴板为空或格式不受支持".into();
                self.formats.clear();
                cx.notify();
                return;
            };

            self.formats = item
                .formats()
                .enumerate()
                .map(|(index, format)| {
                    let preview = format.text().unwrap_or("<二进制负载>");
                    SharedString::from(format!(
                        "富负载 #{}：{} = {}",
                        index + 1,
                        format.mime_type,
                        preview
                    ))
                })
                .collect();

            let plain = item.text().unwrap_or_else(|| "<无纯文本>".to_string());
            self.status = format!("纯文本：{plain}").into();
            cx.notify();
        }
    }

    impl Render for RichClipboardApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 复制用的三种项目：HTML、RTF、仅 CSV。
            let html_item = ClipboardItem::new_html(SAMPLE_HTML, Some(SAMPLE_TEXT.to_string()));
            let rtf_item = ClipboardItem::new_rtf(SAMPLE_RTF, Some(SAMPLE_TEXT.to_string()));
            let csv_item = ClipboardItem::new_with_format(ClipboardFormat::new_text(
                ClipboardFormat::CSV,
                SAMPLE_CSV,
            ));

            let handle = self.self_handle.clone();
            let copy_html = handle.clone();
            let copy_rtf = handle.clone();
            let copy_csv = handle.clone();
            let paste = handle;

            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(div().text_xl().child("富剪贴板示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_sm()
                        .child("源内容：rgpui 富剪贴板（HTML 中 b/i 已标记）"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("copy-html")
                                .label("复制为 HTML + 纯文本")
                                .on_click({
                                    let item = html_item.clone();
                                    move |_event, _window, cx| {
                                        copy_html.update(cx, |this, cx| {
                                            this.copy(item.clone(), "HTML + 纯文本", cx)
                                        });
                                    }
                                }),
                        )
                        .child(
                            Button::new("copy-rtf")
                                .label("复制为 RTF + 纯文本")
                                .on_click({
                                    let item = rtf_item.clone();
                                    move |_event, _window, cx| {
                                        copy_rtf.update(cx, |this, cx| {
                                            this.copy(item.clone(), "RTF + 纯文本", cx)
                                        });
                                    }
                                }),
                        )
                        .child(Button::new("copy-csv").label("复制为 CSV").on_click({
                            let item = csv_item.clone();
                            move |_event, _window, cx| {
                                copy_csv.update(cx, |this, cx| this.copy(item.clone(), "CSV", cx));
                            }
                        }))
                        .child(Button::new("paste").label("粘贴并列出格式").on_click(
                            move |_event, _window, cx| {
                                paste.update(cx, |this, cx| this.paste(cx));
                            },
                        )),
                )
                .children(self.formats.iter().cloned().map(|line| {
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .text_sm()
                        .child(line)
                }))
                .child(div().text_xs().child(format!(
                    "待复制条目数：HTML {} / RTF {} / CSV {}",
                    html_item.entries().len(),
                    rtf_item.entries().len(),
                    csv_item.entries().len()
                )))
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(720.), px(460.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Rich Clipboard Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(RichClipboardApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(RichClipboardApp::new).into()
    }
}

mod text {

    //! 文本排版示例：展示字体族、字号、行高、粗细与颜色等文本样式。
    //! 用法：左侧 tab 选「text」，在窗口里对照不同文字样式的渲染效果。
    use std::{
        // borrow::Cow,
        ops::{Deref, DerefMut},
        sync::Arc,
    };

    use rgpui::{
        AbsoluteLength, App, Context, DefiniteLength, ElementId, Global, Hsla, Menu, SharedString,
        TextStyle, TitlebarOptions, Window, WindowBounds, WindowOptions, bounds,
        colors::DefaultColors, div, point, prelude::*, px, relative, rgb, size,
    };
    use rgpui_platform::application;
    use std::iter;

    #[derive(Clone, Debug)]
    pub struct TextContext {
        font_size: f32,
        line_height: f32,
        type_scale: f32,
    }

    impl Default for TextContext {
        fn default() -> Self {
            TextContext {
                font_size: 16.0,
                line_height: 1.3,
                type_scale: 1.33,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct GlobalTextContext(pub Arc<TextContext>);

    impl Deref for GlobalTextContext {
        type Target = Arc<TextContext>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for GlobalTextContext {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl Global for GlobalTextContext {}

    pub trait ActiveTextContext {
        fn text_context(&self) -> &Arc<TextContext>;
    }

    impl ActiveTextContext for App {
        fn text_context(&self) -> &Arc<TextContext> {
            &self.global::<GlobalTextContext>().0
        }
    }

    #[derive(Clone, PartialEq)]
    pub struct SpecimenTheme {
        pub bg: Hsla,
        pub fg: Hsla,
    }

    impl Default for SpecimenTheme {
        fn default() -> Self {
            Self {
                bg: rgpui::white(),
                fg: rgpui::black(),
            }
        }
    }

    impl SpecimenTheme {
        pub fn invert(&self) -> Self {
            Self {
                bg: self.fg,
                fg: self.bg,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, IntoElement)]
    struct Specimen {
        id: ElementId,
        scale: f32,
        text_style: Option<TextStyle>,
        string: SharedString,
        invert: bool,
    }

    impl Specimen {
        pub fn new(id: usize) -> Self {
            let string = SharedString::new_static("The quick brown fox jumps over the lazy dog");
            let id_string = format!("specimen-{}", id);
            let id = ElementId::Name(id_string.into());
            Self {
                id,
                scale: 1.0,
                text_style: None,
                string,
                invert: false,
            }
        }

        pub fn invert(mut self) -> Self {
            self.invert = !self.invert;
            self
        }

        pub fn scale(mut self, scale: f32) -> Self {
            self.scale = scale;
            self
        }
    }

    impl RenderOnce for Specimen {
        fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
            let rem_size = window.rem_size();
            let scale = self.scale;
            let global_style = cx.text_context();

            let style_override = self.text_style;

            let mut font_size = global_style.font_size;
            let mut line_height = global_style.line_height;

            if let Some(style_override) = style_override {
                font_size = style_override.font_size.to_pixels(rem_size).into();
                line_height = match style_override.line_height {
                    DefiniteLength::Absolute(absolute_len) => match absolute_len {
                        AbsoluteLength::Rems(absolute_len) => {
                            absolute_len.to_pixels(rem_size).into()
                        }
                        AbsoluteLength::Pixels(absolute_len) => absolute_len.into(),
                    },
                    DefiniteLength::Fraction(value) => value,
                };
            }

            let mut theme = SpecimenTheme::default();

            if self.invert {
                theme = theme.invert();
            }

            div()
                .id(self.id)
                .bg(theme.bg)
                .text_color(theme.fg)
                .text_size(px(font_size * scale))
                .line_height(relative(line_height))
                .p(px(10.0))
                .child(self.string)
        }
    }

    #[derive(Debug, Clone, PartialEq, IntoElement)]
    struct CharacterGrid {
        scale: f32,
        invert: bool,
        text_style: Option<TextStyle>,
    }

    impl CharacterGrid {
        pub fn new() -> Self {
            Self {
                scale: 1.0,
                invert: false,
                text_style: None,
            }
        }

        pub fn scale(mut self, scale: f32) -> Self {
            self.scale = scale;
            self
        }
    }

    impl RenderOnce for CharacterGrid {
        fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
            let mut theme = SpecimenTheme::default();

            if self.invert {
                theme = theme.invert();
            }

            let characters = vec![
                "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "A", "B", "C", "D", "E", "F",
                "G", "H", "I", "J", "K", "L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W",
                "X", "Y", "Z", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
                "n", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "ẞ", "ſ", "ß", "ð",
                "Þ", "þ", "α", "β", "Γ", "γ", "Δ", "δ", "η", "θ", "ι", "κ", "Λ", "λ", "μ", "ν",
                "ξ", "π", "τ", "υ", "φ", "χ", "ψ", "∂", "а", "в", "Ж", "ж", "З", "з", "К", "к",
                "л", "м", "Н", "н", "Р", "р", "У", "у", "ф", "ч", "ь", "ы", "Э", "э", "Я", "я",
                "ij", "öẋ", ".,", "⣝⣑", "~", "*", "_", "^", "`", "'", "(", "{", "«", "#", "&", "@",
                "$", "¢", "%", "|", "?", "¶", "µ", "❮", "<=", "!=", "==", "--", "++", "=>", "->",
                "🏀", "🎊", "😍", "❤️", "👍", "👎",
            ];

            let columns = 20;
            let rows = characters.len().div_ceil(columns);

            let grid_rows = (0..rows).map(|row_idx| {
                let start_idx = row_idx * columns;
                let end_idx = (start_idx + columns).min(characters.len());

                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .children((start_idx..end_idx).map(|i| {
                        div()
                            .text_center()
                            .size(px(62.))
                            .bg(theme.bg)
                            .text_color(theme.fg)
                            .text_size(px(24.0))
                            .line_height(relative(1.0))
                            .child(characters[i])
                    }))
                    .when(end_idx - start_idx < columns, |d| {
                        d.children(
                            iter::repeat_with(|| div().flex_1())
                                .take(columns - (end_idx - start_idx)),
                        )
                    })
            });

            div().p_4().gap_2().flex().flex_col().children(grid_rows)
        }
    }

    struct TextExample {
        next_id: usize,
        font_family: SharedString,
    }

    impl TextExample {
        fn next_id(&mut self) -> usize {
            self.next_id += 1;
            self.next_id
        }

        fn button(
            text: &str,
            cx: &mut Context<Self>,
            on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        ) -> impl IntoElement {
            div()
                .id(text.to_string())
                .flex_none()
                .child(text.to_string())
                .bg(rgpui::black())
                .text_color(rgpui::white())
                .active(|this| this.opacity(0.8))
                .px_3()
                .py_1()
                .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
        }
    }

    const FONT_FAMILIES: [&str; 5] = [
        ".ZedMono",
        ".SystemUIFont",
        "Menlo",
        "Monaco",
        "Courier New",
    ];

    impl Render for TextExample {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let tcx = cx.text_context();
            let colors = cx.default_colors().clone();

            let type_scale = tcx.type_scale;

            let step_down_2 = 1.0 / (type_scale * type_scale);
            let step_down_1 = 1.0 / type_scale;
            let base = 1.0;
            let step_up_1 = base * type_scale;
            let step_up_2 = step_up_1 * type_scale;
            let step_up_3 = step_up_2 * type_scale;
            let step_up_4 = step_up_3 * type_scale;
            let step_up_5 = step_up_4 * type_scale;
            let step_up_6 = step_up_5 * type_scale;

            div()
                .font_family(self.font_family.clone())
                .size_full()
                .child(
                    div()
                        .bg(rgpui::white())
                        .border_b_1()
                        .border_color(rgpui::black())
                        .p_3()
                        .flex()
                        .child(Self::button(&self.font_family, cx, |this, cx| {
                            let new_family = FONT_FAMILIES
                                .iter()
                                .position(|f| *f == this.font_family.as_str())
                                .map(|idx| FONT_FAMILIES[(idx + 1) % FONT_FAMILIES.len()])
                                .unwrap_or(FONT_FAMILIES[0]);

                            this.font_family = SharedString::new(new_family);
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("text-example")
                        .overflow_y_scroll()
                        .overflow_x_hidden()
                        .bg(rgb(0xffffff))
                        .size_full()
                        .child(div().child(CharacterGrid::new().scale(base)))
                        .child(
                            div()
                                .child(Specimen::new(self.next_id()).scale(step_down_2))
                                .child(Specimen::new(self.next_id()).scale(step_down_2).invert())
                                .child(Specimen::new(self.next_id()).scale(step_down_1))
                                .child(Specimen::new(self.next_id()).scale(step_down_1).invert())
                                .child(Specimen::new(self.next_id()).scale(base))
                                .child(Specimen::new(self.next_id()).scale(base).invert())
                                .child(Specimen::new(self.next_id()).scale(step_up_1))
                                .child(Specimen::new(self.next_id()).scale(step_up_1).invert())
                                .child(Specimen::new(self.next_id()).scale(step_up_2))
                                .child(Specimen::new(self.next_id()).scale(step_up_2).invert())
                                .child(Specimen::new(self.next_id()).scale(step_up_3))
                                .child(Specimen::new(self.next_id()).scale(step_up_3).invert())
                                .child(Specimen::new(self.next_id()).scale(step_up_4))
                                .child(Specimen::new(self.next_id()).scale(step_up_4).invert())
                                .child(Specimen::new(self.next_id()).scale(step_up_5))
                                .child(Specimen::new(self.next_id()).scale(step_up_5).invert())
                                .child(Specimen::new(self.next_id()).scale(step_up_6))
                                .child(Specimen::new(self.next_id()).scale(step_up_6).invert()),
                        ),
                )
                .child(div().w(px(240.)).h_full().bg(colors.container))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.set_menus(vec![Menu {
                name: "rgpui 排印".into(),
                disabled: false,
                items: vec![],
            }]);

            // let fonts = [include_bytes!(
            //     "../../../assets/fonts/lilex/Lilex-Regular.ttf"
            // )]
            // .iter()
            // .map(|b| Cow::Borrowed(&b[..]))
            // .collect();

            // _ = cx.text_system().add_fonts(fonts);

            cx.init_colors();
            cx.set_global(GlobalTextContext(Arc::new(TextContext::default())));

            let window = cx
                .open_window(
                    WindowOptions {
                        titlebar: Some(TitlebarOptions {
                            title: Some("rgpui 排印".into()),
                            ..Default::default()
                        }),
                        window_bounds: Some(WindowBounds::Windowed(bounds(
                            point(px(0.0), px(0.0)),
                            size(px(920.), px(720.)),
                        ))),
                        ..Default::default()
                    },
                    |_window, cx| {
                        cx.new(|_cx| TextExample {
                            next_id: 0,
                            font_family: ".ZedMono".into(),
                        })
                    },
                )
                .unwrap();

            window
                .update(cx, |_view, _window, cx| {
                    cx.activate(true);
                })
                .unwrap();
        });
    }
}

mod text_layout {

    //! 文本布局示例：展示段落换行、对齐与行布局缓存。
    //! 用法：左侧 tab 选「text_layout」，查看长文本在不同宽度下的换行表现。
    use rgpui::{
        App, Bounds, Context, FontStyle, FontWeight, StyledText, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    struct HelloWorld {}

    impl Render for HelloWorld {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .bg(rgpui::white())
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .size_full()
                .child(div().child("Text left"))
                .child(div().text_center().child("Text center"))
                .child(div().text_right().child("Text right"))
                .child(div().text_decoration_1().child("Text left (underline)"))
                .child(
                    div()
                        .text_center()
                        .text_decoration_1()
                        .child("Text center (underline)"),
                )
                .child(
                    div()
                        .text_right()
                        .text_decoration_1()
                        .child("Text right (underline)"),
                )
                .child(div().line_through().child("Text left (line_through)"))
                .child(
                    div()
                        .text_center()
                        .line_through()
                        .child("Text center (line_through)"),
                )
                .child(
                    div()
                        .text_right()
                        .line_through()
                        .child("Text right (line_through)"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_between()
                        .child(
                            div()
                                .w(px(400.))
                                .border_1()
                                .border_color(rgpui::blue())
                                .p_1()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_center()
                                .child("A long non-wrapping text align center"),
                        )
                        .child(
                            div()
                                .w_32()
                                .border_1()
                                .border_color(rgpui::blue())
                                .p_1()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_right()
                                .child("100%"),
                        ),
                )
                .child(div().flex().gap_2().justify_between().child(
                    StyledText::new("ABCD").with_highlights([
                        (0..1, FontWeight::EXTRA_BOLD.into()),
                        (2..3, FontStyle::Italic.into()),
                    ]),
                ))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| HelloWorld {}),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HelloWorld {}).into()
    }
}

mod text_wrapper {

    //! 文本换行器示例：展示 `TextWrapper` 按词/字符边界断行。
    //! 用法：左侧 tab 选「text_wrapper」，调整宽度观察断行结果。
    use rgpui::{
        App, Bounds, Context, TextOverflow, Window, WindowBounds, WindowOptions, div, prelude::*,
        px, size,
    };
    use rgpui_platform::application;

    struct HelloWorld {}

    impl Render for HelloWorld {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let text = "The longest word 你好世界这段是中文，こんにちはこの段落は日本語です in any of the major \
            English language dictionaries is pneumonoultramicroscopicsilicovolcanoconiosis, a word that \
            refers to a lung disease contracted from the inhalation of very fine silica particles, \
            a url https://github.com/zed-industries/zed/pull/35724?query=foo&bar=2, \
            specifically from a volcano; medically, it is the same as silicosis.";
            div()
                .id("page")
                .size_full()
                .flex()
                .flex_col()
                .p_2()
                .gap_2()
                .bg(rgpui::white())
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_shrink_0()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .border_1()
                                .border_color(rgpui::red())
                                .text_ellipsis()
                                .child("longer text in flex 1"),
                        )
                        .child(
                            div()
                                .flex()
                                .border_1()
                                .border_color(rgpui::red())
                                .text_ellipsis()
                                .child("short flex"),
                        )
                        .child(
                            div()
                                .overflow_hidden()
                                .border_1()
                                .border_color(rgpui::red())
                                .text_ellipsis()
                                .w_full()
                                .child("A short text in normal div"),
                        ),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .truncate()
                        .border_1()
                        .border_color(rgpui::blue())
                        .child("ELLIPSIS: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .overflow_hidden()
                        .text_ellipsis()
                        .line_clamp(2)
                        .border_1()
                        .border_color(rgpui::blue())
                        .child("ELLIPSIS 2 lines: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .overflow_hidden()
                        .text_overflow(TextOverflow::Truncate("".into()))
                        .border_1()
                        .border_color(rgpui::green())
                        .child("TRUNCATE: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .overflow_hidden()
                        .text_overflow(TextOverflow::Truncate("".into()))
                        .line_clamp(3)
                        .border_1()
                        .border_color(rgpui::green())
                        .child("TRUNCATE 3 lines: ".to_owned() + text),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xl()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .border_1()
                        .border_color(rgpui::black())
                        .child("NOWRAP: ".to_owned() + text),
                )
                .child(div().text_xl().w_full().child(text))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| HelloWorld {}),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HelloWorld {}).into()
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
            "i18n",
            "intl_text",
            "markdown",
            "markdown_stream",
            "rich_text",
            "rich_clipboard",
            "text_layout",
            "text_wrapper",
        ];
        let builds: &[fn(&mut Window, &mut App) -> AnyView] = &[
            i18n::build,
            intl_text::build,
            markdown::build,
            markdown_stream::build,
            rich_text::build,
            rich_clipboard::build,
            text_layout::build,
            text_wrapper::build,
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
            "i18n" => i18n::run_example(),
            "intl_text" => intl_text::run_example(),
            "markdown" => markdown::run_example(),
            "markdown_stream" => markdown_stream::run_example(),
            "markdown_selection" => markdown_selection::run_example(),
            "rich_text" => rich_text::run_example(),
            "rich_clipboard" => rich_clipboard::run_example(),
            "text" => text::run_example(),
            "text_layout" => text_layout::run_example(),
            "text_wrapper" => text_wrapper::run_example(),
            _ => eprintln!("未知 demo：{name}"),
        }
        return;
    }
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        // i18n demo 的语言切换按钮依赖全局 I18nManager，先注册以免点击时 panic。
        let mut i18n = rgpui::i18n::I18nManager::new("zh-CN");
        i18n.load_translations_map(
            "zh-CN",
            std::collections::HashMap::from([
                ("greeting".to_string(), "你好，{name}！".to_string()),
                ("welcome".to_string(), "欢迎使用 rgpui。".to_string()),
            ]),
        );
        i18n.load_translations_map(
            "en",
            std::collections::HashMap::from([
                ("greeting".to_string(), "Hello, {name}!".to_string()),
                ("welcome".to_string(), "Welcome to rgpui!".to_string()),
            ]),
        );
        cx.set_global(i18n);
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
