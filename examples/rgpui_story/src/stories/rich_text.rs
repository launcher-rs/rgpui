//! 富文本示例：标题 / 行内样式 / 行内代码 / 链接 / 引用块 / 批注 / 脚注 / 撤销重做。

use rgpui::prelude::*;
use rgpui::rich_text::{Paragraph, RichText, RichTextDoc, RichTextHistory, Run, RunProps};
use rgpui::{
    Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px,
    v_flex,
};

use super::StoryItem;

/// 富文本故事注册表。
pub fn stories() -> Vec<StoryItem> {
    vec![StoryItem {
        title: "富文本",
        build: |_, cx| cx.new(|cx| RichTextStory::new(cx)).into(),
    }]
}

/// 富文本示例视图。
struct RichTextStory {
    /// 当前文档。
    doc: RichTextDoc,
    /// 撤销 / 重做历史。
    history: RichTextHistory,
    /// 状态提示。
    status: SharedString,
    /// 指向自身的句柄（供回调更新状态）。
    handle: Entity<Self>,
    /// 查找命中的字符区间（渲染为搜索高亮）。
    highlight: Vec<std::ops::Range<usize>>,
}

impl RichTextStory {
    /// 创建示例视图（预置一段带各种样式的文档）。
    fn new(cx: &mut Context<Self>) -> Self {
        let doc = sample_doc();
        Self {
            history: RichTextHistory::new(),
            doc,
            status: "点击批注或链接可收到回调；「加粗」/「替换」可用撤销重做回退。".into(),
            handle: cx.entity(),
            highlight: Vec::new(),
        }
    }

    /// 给指定词应用行内样式（走历史栈，可撤销）。
    fn style_word(&mut self, label: &str, props: RunProps, cx: &mut Context<Self>) {
        let plain = self.doc.plain_text();
        if let Some(range) = char_range(&plain, label) {
            self.history.edit(&mut self.doc, |doc| {
                doc.apply_props(range, |p| *p = props);
            });
            self.status = format!("已对「{label}」应用样式").into();
        } else {
            self.status = format!("未在文档中找到「{label}」").into();
        }
        cx.notify();
    }

    /// 查找「加粗」并高亮命中区间。
    fn find_and_highlight(&mut self, cx: &mut Context<Self>) {
        self.highlight = self.doc.find("加粗", true);
        self.status = format!("命中 {} 处「加粗」", self.highlight.len()).into();
        cx.notify();
    }

    /// 撤销一步。
    fn undo(&mut self, cx: &mut Context<Self>) {
        self.status = if self.history.undo(&mut self.doc) {
            "已撤销".into()
        } else {
            "没有可撤销的操作".into()
        };
        cx.notify();
    }

    /// 重做一步。
    fn redo(&mut self, cx: &mut Context<Self>) {
        self.status = if self.history.redo(&mut self.doc) {
            "已重做".into()
        } else {
            "没有可重做的操作".into()
        };
        cx.notify();
    }
}

impl Render for RichTextStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle.clone();
        let mut buttons = div().flex().flex_row().flex_wrap().gap_2();
        for (label, props) in [
            ("加粗", RunProps::new().bold()),
            ("斜体", RunProps::new().italic()),
            ("下划线", RunProps::new().underline()),
            ("删除线", RunProps::new().strikethrough()),
            ("行内代码", RunProps::new().code()),
        ] {
            let handle = handle.clone();
            let label = label.to_string();
            buttons = buttons.child(
                rgpui::Button::new(format!("rich-style-{label}"))
                    .label(label.clone())
                    .on_click(move |_, _, cx| {
                        handle.update(cx, |this, cx| this.style_word(&label, props, cx));
                    }),
            );
        }
        buttons = buttons
            .child(
                rgpui::Button::new("rich-find")
                    .label("查找并高亮「加粗」")
                    .on_click({
                        let handle = handle.clone();
                        move |_, _, cx| handle.update(cx, |this, cx| this.find_and_highlight(cx))
                    }),
            )
            .child(rgpui::Button::new("rich-undo").label("撤销").on_click({
                let handle = handle.clone();
                move |_, _, cx| handle.update(cx, |this, cx| this.undo(cx))
            }))
            .child(rgpui::Button::new("rich-redo").label("重做").on_click({
                let handle = handle.clone();
                move |_, _, cx| handle.update(cx, |this, cx| this.redo(cx))
            }));

        let annotation_handle = handle.clone();
        let link_handle = handle;

        v_flex()
            .id("rich-text-story")
            .gap(px(12.0))
            .p(px(16.0))
            .child(section_title(
                "富文本（标题 / 样式 / 代码 / 链接 / 引用 / 批注 / 脚注）",
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status.clone()),
            )
            .child(buttons)
            .child(
                div()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        RichText::new(self.doc.clone())
                            .highlight_ranges(self.highlight.clone())
                            .base_font_size(px(14.0))
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
    }
}

/// 构造样例文档：标题 + 段落（多种行内样式 + 链接）+ 引用 + 脚注。
fn sample_doc() -> RichTextDoc {
    let mut doc = RichTextDoc::new();

    let mut title = Paragraph::default();
    title.style.heading = Some(1);
    title.runs = vec![Run::new("rgpui 富文本")];
    doc.paragraphs.push(title);

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
            Run::new("、"),
            Run::with("行内代码", RunProps::new().code()),
            Run::new("，以及 "),
            Run::with_link(
                "链接",
                RunProps::default(),
                "https://github.com/launcher-rs/rgpui",
            ),
            Run::new("。"),
        ],
    };
    doc.paragraphs.push(body);

    let mut quote = Paragraph::new("引用块：带左边线与浅底的段落。");
    quote.style.quote = true;
    doc.paragraphs.push(quote);

    let mut spaced = Paragraph::new("行高与段前距：这一段设置了 1.8 倍行高与 12px 段前距。");
    spaced.style.line_height = Some(1.8);
    spaced.style.space_before = Some(px(12.0));
    doc.paragraphs.push(spaced);

    let note = Paragraph::new("这一段带批注（黄色高亮），点击可拿到批注 id。");
    doc.paragraphs.push(note);
    doc.add_annotation(0..4, "示例批注：这里是补充说明。");

    let footnote = Paragraph::new("脚注引用");
    doc.paragraphs.push(footnote);
    doc.insert_footnote(4, "*", "示例脚注：文末脚注内容。");

    // 表格（单元格复用段落）。
    doc.add_table(
        &["能力", "状态"],
        &[
            vec!["撤销 / 重做", "已支持"],
            vec!["链接 / 批注 / 脚注", "已支持"],
        ],
        vec![
            rgpui::rich_text::TableAlign::Left,
            rgpui::rich_text::TableAlign::Center,
        ],
    );

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

/// 在 `haystack` 中定位 `needle` 的**字符**区间。
fn char_range(haystack: &str, needle: &str) -> Option<std::ops::Range<usize>> {
    let start_byte = haystack.find(needle)?;
    let start = haystack[..start_byte].chars().count();
    let len = needle.chars().count();
    Some(start..start + len)
}

/// 小节标题。
fn section_title(text: &'static str) -> impl IntoElement {
    div()
        .text_lg()
        .font_weight(rgpui::FontWeight::BOLD)
        .child(text)
}
