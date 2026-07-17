//! 组件总览示例程序
//!
//! 本示例展示了 rgpui-component 中所有主要组件的基本用法，包括：
//! - 按钮（Button）：各种样式变体、尺寸、状态
//! - 输入框（Input）：文本输入、密码框、可清除输入
//! - 复选框（Checkbox）和开关（Switch）
//! - 单选框（Radio）和单选组（RadioGroup）
//! - 滑块（Slider）：单值和范围选择
//! - 标签（Label）、徽章（Badge）、标签（Tag）
//! - 进度条（Progress）和进度圈（ProgressCircle）
//! - 分隔线（Separator）
//! - 头像（Avatar）和头像组（AvatarGroup）
//! - 骨架屏（Skeleton）和加载器（Spinner）
//! - 警告条（Alert）
//! - 折叠面板（Accordion）
//! - 面包屑（Breadcrumb）
//! - 分页（Pagination）
//! - 评分（Rating）
//! - 描述列表（DescriptionList）
//! - 步骤条（Stepper）
//! - 链接（Link）
//! - 分组框（GroupBox）
//! - 弹出框（Popover）
//! - 标签页（TabBar）

use rgpui::*;
use rgpui_component::{
    alert::Alert,
    avatar::{Avatar, AvatarGroup},
    badge::Badge,
    breadcrumb::{Breadcrumb, BreadcrumbItem},
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    description_list::DescriptionList,
    group_box::GroupBox,
    input::{Input, InputState},
    label::Label,
    link::Link,
    pagination::Pagination,
    popover::Popover,
    progress::{Progress, ProgressCircle},
    radio::{Radio, RadioGroup},
    rating::Rating,
    scroll::ScrollableElement,
    separator::Separator,
    skeleton::Skeleton,
    spinner::Spinner,
    stepper::{Stepper, StepperItem},
    switch::Switch,
    tab::{Tab, TabBar},
    tag::Tag,
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct ComponentsOverview;

impl ComponentsOverview {
    /// 创建按钮区域
    fn render_button_section(&self) -> impl IntoElement {
        section("按钮 Button").child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Button::new("btn-primary").label("主要按钮").primary())
                .child(Button::new("btn-secondary").label("次要按钮").secondary())
                .child(Button::new("btn-danger").label("危险按钮").danger())
                .child(Button::new("btn-outline").label("轮廓按钮").outline())
                .child(Button::new("btn-loading").label("加载中").loading(true)),
        )
    }

    /// 创建输入框区域
    fn render_input_section(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        section("输入框 Input").child(
            h_flex()
                .gap_4()
                .items_start()
                .child(
                    v_flex()
                        .gap_1()
                        .child(Label::new("基础输入"))
                        .child(Input::new(&cx.new(|cx| InputState::new(window, cx)))),
                )
                .child(
                    v_flex().gap_1().child(Label::new("可清除")).child(
                        Input::new(&cx.new(|cx| InputState::new(window, cx))).cleanable(true),
                    ),
                ),
        )
    }

    /// 创建表单控件区域
    fn render_form_controls_section(&self) -> impl IntoElement {
        section("表单控件").child(
            h_flex()
                .gap_8()
                .items_start()
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("复选框 Checkbox"))
                        .child(Checkbox::new("check-1").label("选项一"))
                        .child(Checkbox::new("check-2").label("选项二").checked(true))
                        .child(Checkbox::new("check-3").label("禁用").disabled(true)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("开关 Switch"))
                        .child(Switch::new("switch-1").label("开启通知"))
                        .child(Switch::new("switch-2").label("自动保存").checked(true))
                        .child(Switch::new("switch-3").label("禁用").disabled(true)),
                )
                .child(
                    v_flex().gap_2().child(Label::new("单选框 Radio")).child(
                        RadioGroup::horizontal("radio-group-1")
                            .child(Radio::new("opt-a").label("选项 A"))
                            .child(Radio::new("opt-b").label("选项 B").checked(true))
                            .child(Radio::new("opt-c").label("选项 C")),
                    ),
                ),
        )
    }

    /// 创建数据展示区域
    fn render_data_display_section(&self) -> impl IntoElement {
        section("数据展示")
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("标签 Label"))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(Label::new("普通标签"))
                            .child(Label::new("次要标签").secondary("这是次要文本")),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("徽章 Badge"))
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(Badge::new().count(5))
                            .child(Badge::new().count(150).max(99))
                            .child(Badge::new().dot()),
                    ),
            )
            .child(
                v_flex().gap_2().child(subsection_title("标签 Tag")).child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(Tag::primary().child("主要"))
                        .child(Tag::secondary().child("次要"))
                        .child(Tag::danger().child("危险"))
                        .child(Tag::success().child("成功"))
                        .child(Tag::warning().child("警告"))
                        .child(Tag::info().child("信息")),
                ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("头像 Avatar"))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(Avatar::new().name("张三"))
                            .child(Avatar::new().name("李四").large())
                            .child(
                                AvatarGroup::new()
                                    .child(Avatar::new().name("用户1"))
                                    .child(Avatar::new().name("用户2"))
                                    .child(Avatar::new().name("用户3")),
                            ),
                    ),
            )
    }

    /// 创建进度指示区域
    fn render_progress_section(&self) -> impl IntoElement {
        section("进度指示").child(
            h_flex()
                .gap_8()
                .items_start()
                .child(
                    v_flex()
                        .gap_2()
                        .w_full()
                        .child(subsection_title("进度条 Progress"))
                        .child(Progress::new("prog-1").value(25.0))
                        .child(Progress::new("prog-2").value(60.0).loading(true))
                        .child(Progress::new("prog-3").value(85.0)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .items_center()
                        .child(subsection_title("进度圈 ProgressCircle"))
                        .child(
                            h_flex()
                                .gap_4()
                                .items_center()
                                .child(ProgressCircle::new("pc-1").value(30.0))
                                .child(ProgressCircle::new("pc-2").value(70.0).loading(true))
                                .child(ProgressCircle::new("pc-3").value(50.0).child("50%")),
                        ),
                ),
        )
    }

    /// 创建反馈组件区域
    fn render_feedback_section(&self) -> impl IntoElement {
        section("反馈组件")
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("警告条 Alert"))
                    .child(Alert::info("info-1", "这是一条信息提示。"))
                    .child(Alert::success("success-1", "操作成功完成！"))
                    .child(Alert::warning("warning-1", "请注意，此操作不可逆。"))
                    .child(Alert::error("error-1", "发生错误，请重试。")),
            )
            .child(
                h_flex()
                    .gap_8()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("骨架屏 Skeleton"))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(Skeleton::new().w_10().h_10().rounded_full())
                                    .child(
                                        v_flex()
                                            .gap_2()
                                            .child(Skeleton::new().w_48().h_4())
                                            .child(Skeleton::new().w_32().h_3()),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(subsection_title("加载器 Spinner"))
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_center()
                                    .child(Spinner::new())
                                    .child(Spinner::new().large()),
                            ),
                    ),
            )
    }

    /// 创建导航组件区域
    fn render_navigation_section(&self) -> impl IntoElement {
        section("导航组件")
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("面包屑 Breadcrumb"))
                    .child(
                        Breadcrumb::new()
                            .child(BreadcrumbItem::new("首页"))
                            .child(BreadcrumbItem::new("产品"))
                            .child(BreadcrumbItem::new("详情")),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("分页 Pagination"))
                    .child(
                        Pagination::new("pagination-1")
                            .current_page(3)
                            .total_pages(10),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("标签页 TabBar"))
                    .child(
                        TabBar::new("tabs-1")
                            .child(Tab::new().label("概览"))
                            .child(Tab::new().label("详情"))
                            .child(Tab::new().label("设置"))
                            .child(Tab::new().label("评论")),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("步骤条 Stepper"))
                    .child(
                        Stepper::new("stepper-1")
                            .item(StepperItem::new().child("填写信息"))
                            .item(StepperItem::new().child("验证身份"))
                            .item(StepperItem::new().child("完成")),
                    ),
            )
    }

    /// 创建其他组件区域
    fn render_other_components_section(&self) -> impl IntoElement {
        section("其他组件")
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("分隔线 Separator"))
                    .child(Separator::horizontal())
                    .child(Separator::horizontal().label("或"))
                    .child(Separator::horizontal_dashed()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("描述列表 DescriptionList"))
                    .child(
                        DescriptionList::new()
                            .item("姓名", "张三", 1)
                            .item("年龄", "28", 1)
                            .item("职业", "软件工程师", 1),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("分组框 GroupBox"))
                    .child(
                        GroupBox::new().title("用户信息").child(
                            v_flex()
                                .gap_2()
                                .p_4()
                                .child(Label::new("用户名: admin"))
                                .child(Label::new("角色: 管理员")),
                        ),
                    ),
            )
            .child(
                v_flex().gap_2().child(subsection_title("链接 Link")).child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(
                            Link::new("link-1")
                                .href("https://github.com")
                                .child("GitHub"),
                        )
                        .child(Link::new("link-2").child("自定义点击")),
                ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(subsection_title("评分 Rating"))
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(Rating::new("rating-1").value(3))
                            .child(Rating::new("rating-2").value(5).max(10)),
                    ),
            )
    }

    /// 创建弹出框区域
    fn render_popover_section(&self) -> impl IntoElement {
        section("弹出框 Popover").child(
            Popover::new("popover-trigger")
                .trigger(Button::new("popover-btn").label("点击弹出").secondary())
                .content(|_, _, _| div().p_4().w_64().child(Label::new("这是弹出框的内容。"))),
        )
    }
}

impl Render for ComponentsOverview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("rgpui-component 组件总览"))
            .child(
                div()
                    .id("components-overview")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_button_section())
                    .child(self.render_input_section(window, cx))
                    .child(self.render_form_controls_section())
                    .child(self.render_data_display_section())
                    .child(self.render_progress_section())
                    .child(self.render_feedback_section())
                    .child(self.render_navigation_section())
                    .child(self.render_other_components_section())
                    .child(self.render_popover_section()),
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

fn main() {
    let app = rgpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        rgpui_component::init(cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(900.), px(800.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| ComponentsOverview);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
