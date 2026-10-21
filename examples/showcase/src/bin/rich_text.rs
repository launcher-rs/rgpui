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

/// 行内样式应用函数。
type StyleApply = fn(&mut RunProps);

/// 正文中可被样式按钮命中的词及其对应样式。
const STYLE_TARGETS: &[(&str, StyleApply, &str)] = &[
    ("加粗", |p| p.bold = true, "加粗"),
    ("斜体", |p| p.italic = true, "斜体"),
    ("下划线", |p| p.underline = true, "下划线"),
    ("删除线", |p| p.strikethrough = true, "删除线"),
];

/// 富文本示例应用。
struct RichTextApp {
    /// 指向自身的句柄，供按钮回调更新状态。
    handle: Entity<Self>,
    /// 当前文档。
    doc: RichTextDoc,
    /// 撤销 / 重做历史。
    history: RichTextHistory,
    /// 最近一次操作的结果提示。
    status: SharedString,
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
                Button::new("replace-all")
                    .label("替换「项目符号」→「条目」")
                    .on_click({
                        let handle = handle.clone();
                        move |_event, _window, cx| {
                            handle.update(cx, |this, cx| this.replace_all("项目符号", "条目", cx))
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

/// 构造样例文档：标题 + 段落（含各类行内样式）+ 项目符号 + 编号。
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

/// 在 `haystack` 中定位 `needle` 的**字符**区间。
fn char_range(haystack: &str, needle: &str) -> Option<Range<usize>> {
    let start_byte = haystack.find(needle)?;
    let start = haystack[..start_byte].chars().count();
    let len = needle.chars().count();
    Some(start..start + len)
}

/// 截断字符串用于预览（按字符，附省略号）。
fn preview(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

fn main() {
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
