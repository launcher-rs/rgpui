//! 缩略图排序器示例
//!
//! 演示 `sorter` 缩略图网格：拖拽缩略图重排（拖拽时显示插入指示线）、缩放、
//! 多选（`Ctrl` 点选 / `Shift` 范围选 / `Ctrl+A` 全选）、键盘导航（方向键移动、
//! `Enter` 激活、`Delete` 请求删除）。重排后由应用按事件真正调整数据顺序。
//!
//! 运行：`cargo run -p showcase --bin sorter`

use rgpui::sorter::{Sorter, SorterDelegate, SorterEvent, SorterState};
use rgpui::{
    App, Bounds, Button, Context, Entity, Hsla, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
    prelude::*, px, size, v_flex,
};
use rgpui_platform::application;

/// 单张幻灯片。
struct Slide {
    /// 标题。
    title: SharedString,
    /// 封面底色。
    color: Hsla,
}

/// 幻灯片数据与渲染委托。
struct SlideDeck {
    /// 幻灯片列表（顺序即缩略图顺序）。
    slides: Vec<Slide>,
}

impl SlideDeck {
    /// 构造若干示例幻灯片。
    fn demo() -> Self {
        let slides = (0..12)
            .map(|index| Slide {
                title: format!("第 {} 页", index + 1).into(),
                color: hsla((index as f32 * 0.08) % 1.0, 0.55, 0.55, 1.0),
            })
            .collect();
        Self { slides }
    }
}

impl SorterDelegate for SlideDeck {
    fn count(&self, _cx: &App) -> usize {
        self.slides.len()
    }

    fn aspect_ratio(&self, _cx: &App) -> f32 {
        4.0 / 3.0
    }

    fn render_item(
        &mut self,
        index: usize,
        selected: bool,
        _window: &mut Window,
        _cx: &mut Context<SorterState<Self>>,
    ) -> impl IntoElement {
        let slide = &self.slides[index];
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(slide.color)
            .text_color(hsla(0.0, 0.0, 1.0, 1.0))
            .font_weight(rgpui::FontWeight::BOLD)
            .child(if selected {
                format!("✓ {}", slide.title)
            } else {
                slide.title.to_string()
            })
    }
}

/// 排序器示例应用。
struct SorterApp {
    /// 排序器状态。
    sorter: Entity<SorterState<SlideDeck>>,
    /// 状态栏文案。
    status: SharedString,
}

impl SorterApp {
    /// 创建示例应用并订阅排序器事件。
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sorter = cx.new(|cx| SorterState::new(SlideDeck::demo(), cx));

        cx.subscribe_in(
            &sorter,
            window,
            |this: &mut Self, state, event: &SorterEvent, _window, cx| match event {
                SorterEvent::Reorder { from, to } => {
                    state.update(cx, |state, cx| {
                        let slides = &mut state.delegate_mut().slides;
                        let slide = slides.remove(*from);
                        slides.insert(*to, slide);
                        cx.notify();
                    });
                    this.status = format!("把第 {} 页移动到第 {} 位", from + 1, to + 1).into();
                }
                SorterEvent::SelectionChanged(indices) => {
                    this.status = format!("选中 {} 项", indices.len()).into();
                }
                SorterEvent::Activate(index) => {
                    this.status = format!("激活第 {} 页", index + 1).into();
                }
                SorterEvent::Delete(indices) => {
                    let mut indices = indices.clone();
                    indices.sort_unstable();
                    state.update(cx, |state, cx| {
                        let slides = &mut state.delegate_mut().slides;
                        for index in indices.iter().rev() {
                            if *index < slides.len() {
                                slides.remove(*index);
                            }
                        }
                        cx.notify();
                    });
                    this.status = format!("删除 {} 页", indices.len()).into();
                }
            },
        )
        .detach();

        Self {
            sorter,
            status: "拖拽缩略图重排 · Ctrl 点选 · Shift 范围选 · Delete 删除".into(),
        }
    }

    /// 追加一张幻灯片。
    fn add_slide(&mut self, cx: &mut Context<Self>) {
        self.sorter.update(cx, |state, cx| {
            let deck = state.delegate_mut();
            let number = deck.slides.len() + 1;
            deck.slides.push(Slide {
                title: format!("第 {number} 页").into(),
                color: hsla((number as f32 * 0.08) % 1.0, 0.55, 0.55, 1.0),
            });
            cx.notify();
        });
        self.status = "已新建一页".into();
    }

    /// 删除当前选中项。
    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self.sorter.read(cx).selected().iter().copied().collect();
        if indices.is_empty() {
            self.status = "没有选中项".into();
            return;
        }
        self.sorter.update(cx, |state, cx| {
            let slides = &mut state.delegate_mut().slides;
            for index in indices.iter().rev() {
                if *index < slides.len() {
                    slides.remove(*index);
                }
            }
            cx.notify();
        });
        self.status = format!("删除 {} 页", indices.len()).into();
    }
}

impl Render for SorterApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom_out = {
            let sorter = self.sorter.clone();
            Button::new("zoom-out").label("缩小").on_click(cx.listener(
                move |_this, _event, _window, cx| {
                    sorter.update(cx, |state, cx| state.zoom_by(-0.2, cx));
                },
            ))
        };
        let zoom_in = {
            let sorter = self.sorter.clone();
            Button::new("zoom-in").label("放大").on_click(cx.listener(
                move |_this, _event, _window, cx| {
                    sorter.update(cx, |state, cx| state.zoom_by(0.2, cx));
                },
            ))
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(zoom_out)
                    .child(zoom_in)
                    .child(
                        Button::new("add")
                            .label("新建")
                            .on_click(cx.listener(|this, _event, _window, cx| this.add_slide(cx))),
                    )
                    .child(Button::new("delete").label("删除选中").on_click(
                        cx.listener(|this, _event, _window, cx| this.delete_selected(cx)),
                    ))
                    .child(div().text_sm().child(self.status.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(Sorter::new(&self.sorter)),
            )
    }
}

fn main() {
    rgpui::init_logging();
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(640.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Sorter Example".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| SorterApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
