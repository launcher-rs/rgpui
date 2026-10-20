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

/// 示例正文：一段需要加粗/斜体标记的内容
const SAMPLE_TEXT: &str = "rgpui 富剪贴板";
/// 与 [`SAMPLE_TEXT`] 对应的 HTML 片段
const SAMPLE_HTML: &str = "<p><b>rgpui</b> <i>富剪贴板</i></p>";
/// 与 [`SAMPLE_TEXT`] 对应的 RTF 片段
const SAMPLE_RTF: &str = r"{\rtf1\ansi rgpui 富剪贴板}";
/// 独立演示自定义 MIME（CSV，无纯文本降级）。
const SAMPLE_CSV: &str = "name,qty\napple,3\npear,5";

/// 富剪贴板示例应用
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

fn main() {
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
