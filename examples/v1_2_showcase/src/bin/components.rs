//! 新组件演示：Upload + Carousel + Mermaid + Sidebar 分组 + B 系列小件，一页展示。
//!
//! B 系列：Select / Combobox / DatePicker / ColorPicker / Avatar / Alert /
//! Breadcrumb / Card / Typography / Pagination / Steps / Timeline / Rate /
//! Toggle(+Group) / Popconfirm。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    Alert, AlertVariant, App, Avatar, AvatarStatus, Bounds, Breadcrumb, BreadcrumbItem, Card,
    Context, Hsla, IconName, Link, Paragraph, Popconfirm, Render, ScrollHandle, TextVariant, Title,
    TitleLevel, Toggle, ToggleGroup, Window, WindowBounds, WindowOptions,
    components::{
        Carousel, CarouselState, ColorPickerState, ComboboxState, DatePickerState, MermaidDiagram,
        Pagination, Rate, Select, Sidebar, SidebarItem, SidebarSection, StepItem, Steps, Timeline,
        TimelineItem, UploadState,
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
    // B 系列演示状态。
    select_idx: Option<usize>,
    combo: rgpui::Entity<ComboboxState>,
    combo_status: String,
    date: rgpui::Entity<DatePickerState>,
    color: rgpui::Entity<ColorPickerState>,
    color_value: Hsla,
    alert_visible: bool,
    crumb: String,
    page: usize,
    steps_current: usize,
    rate: f32,
    toggle_on: bool,
    toggle_idx: Option<usize>,
    confirm_status: String,
}

impl ComponentsDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let upload = cx.new(|cx| UploadState::new(window, cx));
        let carousel = cx.new(|_| CarouselState::new().autoplay(std::time::Duration::from_secs(3)));
        let demo = cx.entity();
        let combo = cx.new(|cx| {
            ComboboxState::new(window, cx)
                .items(vec![
                    "apple".into(),
                    "apricot".into(),
                    "banana".into(),
                    "blueberry".into(),
                ])
                .on_change({
                    let demo = demo.clone();
                    move |selected: &[usize], _, cx| {
                        let selected = selected.to_vec();
                        demo.update(cx, |this, _| {
                            this.combo_status = format!("已选下标：{selected:?}");
                        })
                    }
                })
        });
        let date = cx.new(|_| DatePickerState::new());
        let color = cx.new(|cx| {
            ColorPickerState::new(window, cx).on_change({
                let demo = demo.clone();
                move |color: Hsla, _, cx| {
                    demo.update(cx, |this, _| {
                        this.color_value = color;
                    })
                }
            })
        });
        Self {
            upload,
            carousel,
            scroll: ScrollHandle::new(),
            selected: "inbox".into(),
            collapsed_sections: HashSet::new(),
            sidebar_collapsed: false,
            sidebar_hidden: false,
            select_idx: None,
            combo,
            combo_status: "输入过滤，下拉多选一".to_string(),
            date,
            color,
            color_value: Hsla::default(),
            alert_visible: true,
            crumb: "未点击".to_string(),
            page: 1,
            steps_current: 1,
            rate: 3.0,
            toggle_on: false,
            toggle_idx: None,
            confirm_status: "未确认".to_string(),
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
                    )
                    .child(div().text_xl().child("Select（简单值列表）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(
                                Select::new(vec!["苹果".into(), "香蕉".into(), "樱桃".into()])
                                    .selected(self.select_idx)
                                    .placeholder("选一个水果")
                                    .on_change({
                                        let demo = demo.clone();
                                        move |ix: usize, _: &rgpui::SharedString, _, cx| {
                                            demo.update(cx, |this, _| {
                                                this.select_idx = Some(ix);
                                            })
                                        }
                                    }),
                            )
                            .child(div().text_sm().child(format!(
                                "选中：{}",
                                self.select_idx
                                    .and_then(|ix| ["苹果", "香蕉", "樱桃"].get(ix))
                                    .unwrap_or(&"无")
                            ))),
                    )
                    .child(div().text_xl().child("Combobox（可搜索下拉）"))
                    .child(self.combo.clone())
                    .child(div().text_sm().child(self.combo_status.clone()))
                    .child(div().text_xl().child("DatePicker（日历弹窗）"))
                    .child(self.date.clone())
                    .child(div().text_sm().child(format!(
                            "已选：{}",
                            self.date
                                .read_with(cx, |state, _| state
                                    .selected()
                                    .map(|d| d.format("%Y-%m-%d").to_string()))
                                .as_deref()
                                .unwrap_or("未选择")
                        )))
                    .child(div().text_xl().child("ColorPicker（取色）"))
                    .child(self.color.clone())
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(
                                div()
                                    .w(px(48.0))
                                    .h(px(24.0))
                                    .rounded_md()
                                    .bg(self.color_value),
                            )
                            .child(div().text_sm().child(format!("{:?}", self.color_value))),
                    )
                    .child(div().text_xl().child("Avatar（头像 + 状态）"))
                    .child(
                        h_flex()
                            .gap(px(12.0))
                            .items_center()
                            .child(
                                Avatar::new("RG")
                                    .size(px(40.0))
                                    .status(AvatarStatus::Online),
                            )
                            .child(Avatar::new("AB").size(px(40.0)).status(AvatarStatus::Busy))
                            .child(Avatar::new("CD").size(px(40.0))),
                    )
                    .child(div().text_xl().child("Alert（行内提示条）"))
                    .child(Alert::new("信息提示").body("这是 info 变体。"))
                    .child(
                        Alert::new("操作成功")
                            .variant(AlertVariant::Success)
                            .body("这是 success 变体。"),
                    )
                    .child(
                        Alert::new("注意")
                            .variant(AlertVariant::Warning)
                            .body("这是 warning 变体。"),
                    )
                    .child(
                        Alert::new("出错了")
                            .variant(AlertVariant::Danger)
                            .body("这是 danger 变体。"),
                    )
                    .children(if self.alert_visible {
                        Some(
                            Alert::new("可关闭")
                                .variant(AlertVariant::Info)
                                .closable(true)
                                .on_close({
                                    let demo = demo.clone();
                                    move |_, cx| {
                                        demo.update(cx, |this, _| {
                                            this.alert_visible = false;
                                        })
                                    }
                                }),
                        )
                    } else {
                        None
                    })
                    .child(
                        h_flex().gap(px(8.0)).items_center().child(
                            rgpui::Button::new("alert-restore")
                                .label("恢复可关闭提示条")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.alert_visible = true;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(div().text_xl().child("Breadcrumb（路径导航）"))
                    .child(Breadcrumb::new(vec![
                        BreadcrumbItem::new("首页").on_click({
                            let demo = demo.clone();
                            move |_, cx| {
                                demo.update(cx, |this, _| {
                                    this.crumb = "首页".to_string();
                                })
                            }
                        }),
                        BreadcrumbItem::new("组件"),
                        BreadcrumbItem::new("当前页"),
                    ]))
                    .child(div().text_sm().child(format!("点击：{}", self.crumb)))
                    .child(div().text_xl().child("Card（卡片容器）"))
                    .child(
                        Card::new().title("卡片标题").child(
                            div()
                                .text_sm()
                                .child("卡片内容区（header/content/footer 由子元素拼）。"),
                        ),
                    )
                    .child(div().text_xl().child("Typography（排印）"))
                    .child(Title::new("二级标题").level(TitleLevel::H2))
                    .child(Paragraph::new("次要正文（灰）。").variant(TextVariant::Secondary))
                    .child(Link::new("点我也是面包屑").on_click({
                        let demo = demo.clone();
                        move |_, cx| {
                            demo.update(cx, |this, _| {
                                this.crumb = "链接".to_string();
                            })
                        }
                    }))
                    .child(div().text_xl().child("Pagination（页码导航）"))
                    .child(Pagination::new(self.page, 10).on_change({
                        let demo = demo.clone();
                        move |page: usize, _, cx| {
                            demo.update(cx, |this, _| {
                                this.page = page;
                            })
                        }
                    }))
                    .child(div().text_sm().child(format!("当前第 {} 页", self.page)))
                    .child(div().text_xl().child("Steps（步骤条）"))
                    .child(Steps::new(
                        vec![
                            StepItem::new("下单").description("已完成"),
                            StepItem::new("支付"),
                            StepItem::new("收货"),
                        ],
                        self.steps_current,
                    ))
                    .child(
                        h_flex().gap(px(8.0)).child(
                            rgpui::Button::new("steps-next")
                                .label("下一步")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.steps_current = (this.steps_current + 1).min(2);
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(div().text_xl().child("Timeline（时间线）"))
                    .child(Timeline::new(vec![
                        TimelineItem::new("10:24", "需求评审"),
                        TimelineItem::new("11:02", "方案确定"),
                        TimelineItem::new("14:40", "开始实现"),
                    ]))
                    .child(div().text_xl().child("Rate（星级评分）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(Rate::new().value(self.rate).on_change({
                                let demo = demo.clone();
                                move |value: f32, _, cx| {
                                    demo.update(cx, |this, _| {
                                        this.rate = value;
                                    })
                                }
                            }))
                            .child(div().text_sm().child(format!("{:.1} 星", self.rate))),
                    )
                    .child(div().text_xl().child("Toggle（切换按钮）"))
                    .child(
                        h_flex()
                            .gap(px(12.0))
                            .items_center()
                            .child(Toggle::new("飞行模式").pressed(self.toggle_on).on_change({
                                let demo = demo.clone();
                                move |pressed: bool, _, cx| {
                                    demo.update(cx, |this, _| {
                                        this.toggle_on = pressed;
                                    })
                                }
                            }))
                            .child(
                                ToggleGroup::new(vec!["日".into(), "周".into(), "月".into()])
                                    .selected(self.toggle_idx)
                                    .on_change({
                                        let demo = demo.clone();
                                        move |ix: usize, _, cx| {
                                            demo.update(cx, |this, _| {
                                                this.toggle_idx = Some(ix);
                                            })
                                        }
                                    }),
                            )
                            .child(div().text_sm().child(format!(
                                "开关：{} / 分组：{}",
                                if self.toggle_on { "开" } else { "关" },
                                self.toggle_idx
                                    .and_then(|ix| ["日", "周", "月"].get(ix))
                                    .unwrap_or(&"无"),
                            ))),
                    )
                    .child(div().text_xl().child("Popconfirm（气泡确认）"))
                    .child(
                        h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(
                                Popconfirm::new("删除", "确定删除这条记录吗？")
                                    .on_confirm({
                                        let demo = demo.clone();
                                        move |_, cx| {
                                            demo.update(cx, |this, _| {
                                                this.confirm_status = "已确认删除".to_string();
                                            })
                                        }
                                    })
                                    .on_cancel({
                                        let demo = demo.clone();
                                        move |_, cx| {
                                            demo.update(cx, |this, _| {
                                                this.confirm_status = "已取消".to_string();
                                            })
                                        }
                                    }),
                            )
                            .child(div().text_sm().child(self.confirm_status.clone())),
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
