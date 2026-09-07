//! 新组件演示：Upload + Carousel + Mermaid + Sidebar 分组，一页展示。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    App, Bounds, Context, IconName, Render, ScrollHandle, Window, WindowBounds, WindowOptions,
    components::{
        Carousel, CarouselState, MermaidDiagram, Sidebar, SidebarItem, SidebarSection, UploadState,
    },
    div, h_flex,
    prelude::*,
    px, rgb, size, v_flex,
};
use rgpui_platform::application;
use std::collections::HashSet;

const MERMAID_SAMPLE: &str =
    "flowchart LR\nA[需求] -->|评审| B{方案}\nB --> C[实现]\nB --> D(暂缓)";

struct ComponentsDemo {
    upload: rgpui::Entity<UploadState>,
    carousel: rgpui::Entity<CarouselState>,
    scroll: ScrollHandle,
    selected: rgpui::SharedString,
    collapsed_sections: HashSet<String>,
    sidebar_collapsed: bool,
    sidebar_hidden: bool,
}

impl ComponentsDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let upload = cx.new(|cx| UploadState::new(window, cx));
        let carousel = cx.new(|_| CarouselState::new().autoplay(std::time::Duration::from_secs(3)));
        Self {
            upload,
            carousel,
            scroll: ScrollHandle::new(),
            selected: "inbox".into(),
            collapsed_sections: HashSet::new(),
            sidebar_collapsed: false,
            sidebar_hidden: false,
        }
    }
}

impl Render for ComponentsDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        let collapsed = self.collapsed_sections.clone();
        let scroll = self.scroll.clone();
        let demo = cx.entity();

        div()
            .id("components-page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .child(
                v_flex()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .child(div().text_xl().child("Upload（文件上传）"))
                    .child(self.upload.clone())
                    .child(div().text_xl().child("Carousel（轮播）"))
                    .child(
                        Carousel::new(self.carousel.clone())
                            .child(
                                div()
                                    .h(px(120.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(rgb(0x3b82f6))
                                    .text_xl()
                                    .child("第一页"),
                            )
                            .child(
                                div()
                                    .h(px(120.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(rgb(0x10b981))
                                    .text_xl()
                                    .child("第二页"),
                            )
                            .child(
                                div()
                                    .h(px(120.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(rgb(0xf59e0b))
                                    .text_xl()
                                    .child("第三页"),
                            ),
                    )
                    .child(div().text_xl().child("Mermaid（流程图）"))
                    .child(MermaidDiagram::new(MERMAID_SAMPLE))
                    .child(div().text_xl().child("Sidebar（分组 + 角标 + 折叠/隐藏）"))
                    .child(
                        h_flex().gap(px(8.0)).child(
                            rgpui::Button::new("sidebar-hide")
                                .label(if self.sidebar_hidden {
                                    "显示侧边栏"
                                } else {
                                    "隐藏侧边栏"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sidebar_hidden = !this.sidebar_hidden;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        h_flex()
                            .h(px(280.0))
                            .when(!self.sidebar_hidden, |this| {
                                let collapsed_handle = demo.clone();
                                let section_handle = demo.clone();
                                this.child(
                                    Sidebar::new()
                                        .collapsible(true)
                                        .collapsed(self.sidebar_collapsed)
                                        .on_toggle_collapsed(move |collapsed, _, cx| {
                                            let collapsed = *collapsed;
                                            collapsed_handle.update(cx, |this, _| {
                                                this.sidebar_collapsed = collapsed;
                                            });
                                        })
                                        .item(
                                            SidebarItem::new("inbox", "收件箱")
                                                .with_icon(IconName::File)
                                                .with_badge("12"),
                                        )
                                        .section(
                                            SidebarSection::new(
                                                "项目",
                                                vec![
                                                    SidebarItem::new("rgpui", "rgpui")
                                                        .with_badge("3"),
                                                    SidebarItem::new("editor", "ru_editor"),
                                                ],
                                            )
                                            .collapsed(collapsed.contains("项目")),
                                        )
                                        .selected(selected)
                                        .on_select(cx.listener(
                                            |this, id: &rgpui::SharedString, _, _| {
                                                this.selected = id.clone();
                                            },
                                        ))
                                        .on_toggle_section(move |title, collapsed, _, cx| {
                                            let title = title.clone();
                                            let collapsed = *collapsed;
                                            section_handle.update(cx, |this, _| {
                                                if collapsed {
                                                    this.collapsed_sections
                                                        .insert(title.to_string());
                                                } else {
                                                    this.collapsed_sections.remove(title.as_str());
                                                }
                                            });
                                        }),
                                )
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .p(px(16.0))
                                    .child("右侧内容区（随左侧选择切换，此处略）。"),
                            ),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(860.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ComponentsDemo::new(window, cx)),
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
