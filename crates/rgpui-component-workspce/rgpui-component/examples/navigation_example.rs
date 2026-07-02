//! 导航组件示例程序
//!
//! 本示例详细展示了导航组件的用法，包括：
//! - 标签页（TabBar）：基础标签页、不同样式变体
//! - 面包屑（Breadcrumb）：层级导航
//! - 分页（Pagination）：页面导航
//! - 步骤条（Stepper）：流程步骤指示

use rgpui::*;
use rgpui_component::{
    breadcrumb::{Breadcrumb, BreadcrumbItem},
    pagination::Pagination,
    scroll::ScrollableElement,
    stepper::{Stepper, StepperItem},
    tab::{Tab, TabBar},
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct NavigationExample;

impl NavigationExample {
    /// 渲染标签页区域
    fn render_tabs(&self) -> impl IntoElement {
        section("标签页 TabBar")
            .child(description("标签页用于在不同内容视图之间切换。"))
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        v_flex().gap_2().child(subsection_title("默认样式")).child(
                            TabBar::new("tabs-default")
                                .child(Tab::new().label("概览"))
                                .child(Tab::new().label("详情"))
                                .child(Tab::new().label("设置"))
                                .child(Tab::new().label("评论")),
                        ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("胶囊样式 Pill"))
                            .child(
                                TabBar::new("tabs-pill")
                                    .pill()
                                    .child(Tab::new().label("概览"))
                                    .child(Tab::new().label("详情"))
                                    .child(Tab::new().label("设置")),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("轮廓样式 Outline"))
                            .child(
                                TabBar::new("tabs-outline")
                                    .outline()
                                    .child(Tab::new().label("概览"))
                                    .child(Tab::new().label("详情"))
                                    .child(Tab::new().label("设置")),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("分段样式 Segmented"))
                            .child(
                                TabBar::new("tabs-segmented")
                                    .segmented()
                                    .child(Tab::new().label("日"))
                                    .child(Tab::new().label("周"))
                                    .child(Tab::new().label("月"))
                                    .child(Tab::new().label("年")),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("下划线样式 Underline"))
                            .child(
                                TabBar::new("tabs-underline")
                                    .underline()
                                    .child(Tab::new().label("主页"))
                                    .child(Tab::new().label("博客"))
                                    .child(Tab::new().label("关于")),
                            ),
                    )
                    .child(
                        v_flex().gap_2().child(subsection_title("禁用状态")).child(
                            TabBar::new("tabs-disabled")
                                .child(Tab::new().label("可用"))
                                .child(Tab::new().label("禁用").disabled(true))
                                .child(Tab::new().label("可用")),
                        ),
                    ),
            )
    }

    /// 渲染面包屑区域
    fn render_breadcrumbs(&self) -> impl IntoElement {
        section("面包屑 Breadcrumb")
            .child(description("面包屑用于显示当前页面的层级路径。"))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_flex().gap_2().child(subsection_title("基础用法")).child(
                            Breadcrumb::new()
                                .child(BreadcrumbItem::new("首页"))
                                .child(BreadcrumbItem::new("产品"))
                                .child(BreadcrumbItem::new("详情")),
                        ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("带点击事件"))
                            .child(
                                Breadcrumb::new()
                                    .child(BreadcrumbItem::new("首页").on_click(|_, _, _| {
                                        println!("点击了首页");
                                    }))
                                    .child(BreadcrumbItem::new("产品"))
                                    .child(BreadcrumbItem::new("详情")),
                            ),
                    )
                    .child(
                        v_flex().gap_2().child(subsection_title("禁用状态")).child(
                            Breadcrumb::new()
                                .child(BreadcrumbItem::new("首页"))
                                .child(BreadcrumbItem::new("产品").disabled(true))
                                .child(BreadcrumbItem::new("详情")),
                        ),
                    ),
            )
    }

    /// 渲染分页区域
    fn render_pagination(&self) -> impl IntoElement {
        section("分页 Pagination")
            .child(description("分页用于在大量数据中分页浏览。"))
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        v_flex().gap_2().child(subsection_title("基础用法")).child(
                            Pagination::new("pagination-1")
                                .current_page(3)
                                .total_pages(10),
                        ),
                    )
                    .child(
                        v_flex().gap_2().child(subsection_title("紧凑模式")).child(
                            Pagination::new("pagination-2")
                                .current_page(5)
                                .total_pages(20)
                                .compact(),
                        ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("显示7个页码"))
                            .child(
                                Pagination::new("pagination-3")
                                    .current_page(10)
                                    .total_pages(50)
                                    .visible_pages(7),
                            ),
                    ),
            )
    }

    /// 渲染步骤条区域
    fn render_stepper(&self) -> impl IntoElement {
        section("步骤条 Stepper")
            .child(description("步骤条用于显示多步骤流程的进度。"))
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("水平步骤条"))
                            .child(
                                Stepper::new("stepper-h")
                                    .item(StepperItem::new().child("填写信息"))
                                    .item(StepperItem::new().child("验证身份"))
                                    .item(StepperItem::new().child("完成")),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("垂直步骤条"))
                            .child(
                                Stepper::new("stepper-v")
                                    .vertical()
                                    .item(StepperItem::new().child("步骤一"))
                                    .item(StepperItem::new().child("步骤二"))
                                    .item(StepperItem::new().child("步骤三")),
                            ),
                    ),
            )
    }
}

impl Render for NavigationExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("导航组件 Navigation 示例"))
            .child(
                div()
                    .id("navigation-examples")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_tabs())
                    .child(self.render_breadcrumbs())
                    .child(self.render_pagination())
                    .child(self.render_stepper()),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

fn section(title: &str) -> Div {
    div()
        .v_flex()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(rgpui::black().opacity(0.1))
        .child(div().text_lg().font_bold().child(title.to_string()))
}

fn subsection_title(title: &str) -> impl IntoElement {
    div()
        .text_sm()
        .font_bold()
        .text_color(rgpui::black().opacity(0.6))
        .child(title.to_string())
}

fn description(text: &str) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(rgpui::black().opacity(0.6))
        .child(text.to_string())
}

fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(900.), px(700.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| NavigationExample);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
