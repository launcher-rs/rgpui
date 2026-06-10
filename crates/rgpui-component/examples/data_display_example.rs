//! 数据展示组件示例程序
//!
//! 本示例详细展示了数据展示组件的用法，包括：
//! - 标签（Label）：普通标签、次要标签
//! - 徽章（Badge）：计数徽章、点徽章、最大值限制
//! - 标签（Tag）：各种颜色变体、轮廓样式
//! - 头像（Avatar）：图片头像、文字头像、头像组
//! - 进度条（Progress）：确定进度、加载状态
//! - 进度圈（ProgressCircle）：圆形进度指示器
//! - 分隔线（Separator）：水平分隔线、带标签分隔线

use rgpui::*;
use rgpui_component::{
    avatar::{Avatar, AvatarGroup},
    badge::Badge,
    label::Label,
    progress::{Progress, ProgressCircle},
    scroll::ScrollableElement,
    separator::Separator,
    tag::Tag,
    *,
};
use rgpui_component_assets::Assets;

/// 示例应用主结构体
pub struct DataDisplayExample;

impl DataDisplayExample {
    /// 渲染标签区域
    fn render_labels(&self) -> impl IntoElement {
        section("标签 Label")
            .child(description("标签用于显示文本信息。"))
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(Label::new("普通标签"))
                    .child(Label::new("次要标签").secondary("这是次要文本")),
            )
    }

    /// 渲染徽章区域
    fn render_badges(&self) -> impl IntoElement {
        section("徽章 Badge")
            .child(description("徽章用于显示计数或状态指示。"))
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(Badge::new().count(5))
                    .child(Badge::new().count(150).max(99))
                    .child(Badge::new().dot())
                    .child(Badge::new().count(10).color(rgpui::green())),
            )
    }

    /// 渲染标签区域
    fn render_tags(&self) -> impl IntoElement {
        section("标签 Tag")
            .child(description("标签用于分类或标记内容。"))
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(Tag::primary().child("主要"))
                    .child(Tag::secondary().child("次要"))
                    .child(Tag::danger().child("危险"))
                    .child(Tag::success().child("成功"))
                    .child(Tag::warning().child("警告"))
                    .child(Tag::info().child("信息")),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(Tag::primary().outline().child("主要"))
                    .child(Tag::secondary().outline().child("次要"))
                    .child(Tag::danger().outline().child("危险"))
                    .child(Tag::success().outline().child("成功")),
            )
    }

    /// 渲染头像区域
    fn render_avatars(&self) -> impl IntoElement {
        section("头像 Avatar")
            .child(description("头像用于显示用户或实体的标识。"))
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(Avatar::new().name("张三"))
                    .child(Avatar::new().name("李四").large())
                    .child(Avatar::new().name("王五").small())
                    .child(
                        AvatarGroup::new()
                            .child(Avatar::new().name("用户1"))
                            .child(Avatar::new().name("用户2"))
                            .child(Avatar::new().name("用户3")),
                    ),
            )
    }

    /// 渲染进度指示区域
    fn render_progress(&self) -> impl IntoElement {
        section("进度指示 Progress")
            .child(description("进度指示器用于显示操作进度。"))
            .child(
                h_flex()
                    .gap_8()
                    .items_start()
                    .child(
                        v_flex()
                            .gap_3()
                            .w_full()
                            .child(subsection_title("进度条"))
                            .child(Progress::new("prog-1").value(25.0))
                            .child(Progress::new("prog-2").value(60.0))
                            .child(Progress::new("prog-3").value(85.0))
                            .child(Progress::new("prog-4").loading(true)),
                    )
                    .child(
                        v_flex()
                            .gap_3()
                            .items_center()
                            .child(subsection_title("进度圈"))
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_center()
                                    .child(ProgressCircle::new("pc-1").value(30.0))
                                    .child(ProgressCircle::new("pc-2").value(70.0))
                                    .child(ProgressCircle::new("pc-3").loading(true))
                                    .child(ProgressCircle::new("pc-4").value(50.0).child("50%")),
                            ),
                    ),
            )
    }

    /// 渲染分隔线区域
    fn render_separators(&self) -> impl IntoElement {
        section("分隔线 Separator")
            .child(description("分隔线用于分隔内容区域。"))
            .child(
                v_flex()
                    .gap_3()
                    .child(Label::new("水平分隔线"))
                    .child(Separator::horizontal())
                    .child(Label::new("带标签的分隔线"))
                    .child(Separator::horizontal().label("或"))
                    .child(Label::new("虚线分隔线"))
                    .child(Separator::horizontal_dashed()),
            )
    }
}

impl Render for DataDisplayExample {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .v_flex()
            .child(TitleBar::new().child("数据展示 Data Display 示例"))
            .child(
                div()
                    .id("data-display-examples")
                    .p_6()
                    .v_flex()
                    .gap_6()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_labels())
                    .child(self.render_badges())
                    .child(self.render_tags())
                    .child(self.render_avatars())
                    .child(self.render_progress())
                    .child(self.render_separators()),
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
                let view = cx.new(|_| DataDisplayExample);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("打开窗口失败");
        })
        .detach();
    });
}
