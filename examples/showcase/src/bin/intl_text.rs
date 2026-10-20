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
    App, Bounds, Context, IntoElement, ParentElement, Pixels, Render, SharedString, Styled as _,
    TextAlign, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

/// 方向判定样例。
struct DirectionSample {
    /// 样本文本。
    text: &'static str,
    /// 样例说明。
    note: &'static str,
}

/// 方向判定样例集（覆盖 LTR / RTL / 弱方向字符 / 混排）。
const DIRECTION_SAMPLES: &[DirectionSample] = &[
    DirectionSample {
        text: "Hello world",
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

/// 断行样例对。
struct PairSample {
    /// 前一个字符。
    prev: char,
    /// 后一个字符。
    next: char,
    /// 判定说明。
    note: &'static str,
}

/// 断行样例集。
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

/// 字符类别样例。
const CLASS_SAMPLES: &[char] = &['汉', 'あ', 'ぁ', 'ー', '。', '「', '한', 'a', ' ', '+'];

/// 东亚禁则样例文本（句读 / 闭引号 / 开引号齐全）。
const CJK_SAMPLE: &str = "他说：「今天的天气很好，我们一起去公园散步吧。」（真的很适合散步。）";

/// 国际文本排版示例视图。
struct IntlTextDemo;

impl IntlTextDemo {
    /// 创建示例视图。
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

/// 小节卡片（标题 + 正文）。
fn section(title: impl Into<SharedString>, body: impl IntoElement, cx: &App) -> impl IntoElement {
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

/// 「段落基准方向」小节：样例文本 + 解析结果（Start 对齐让 RTL 文本自动靠右）。
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

/// 「start / end 对齐」小节：同一段 RTL 文本在四种对齐下的差别。
fn align_section(cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let text = "שלום עולם";
    let alphas = [
        (TextAlign::Left, "Left", "显式左对齐（与文字方向无关）"),
        (
            TextAlign::Start,
            "Start",
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

/// 「东亚禁则」小节：窄容器里的 CJK 换行 + 规则速查。
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

/// 「字符类别」小节：断行规则眼中的字符类别。
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

/// 「语言与语言包」小节：语言标签 → 默认方向 + 语言包缺 key 审计。
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

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(980.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("International Text Example".into()),
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
