//! 创意 / 交互 / 布局 示例集合。
//!
//! 用法：cargo run -p showcase --bin creative_gallery -- <demo>
//! 可用 demo：ink / sorter / timeline / color_picker / mdi / task_pane / canvas_a11y / page_view / drag_drop / focus_visible / mouse_pressure / native_menu / native_notification / set_menus / tab_stop / chat / dock / grid_layout / status_bar / tree
//! 不带参数时默认运行 ink，传 --list 打印清单。

#![cfg_attr(target_family = "wasm", no_main)]

use rgpui::{
    AnyView, App, Bounds, Button, ButtonVariants, Context, IntoElement, ParentElement, Render,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use rgpui_platform::application;

mod ink {
    //! Ink 笔迹示例
    //!
    //! 演示 `ink` 墨迹层：鼠标拖拽用钢笔 / 荧光笔落笔，橡皮按半径擦除，套索框选后删除；
    //! 另含撤销与清空。笔画以中点二次贝塞尔平滑绘制，荧光笔叠加半透明；打开「压感笔」后
    //! 按轨迹长度合成压感，笔画逐段变宽（真实数位板压感走同一接口）。
    //!
    //! 多图层：`InkDocument` 管理图层（新增 / 切换 / 隐藏 / 锁定），编辑落在激活图层，
    //! 可见层逐层绘制并把图层不透明度传给 `InkLayerElement`。
    //!
    //! 运行：`cargo run -p showcase --bin ink`

    use rgpui::ink::{InkDocument, InkLayerElement, InkStyle, InkTool};
    use rgpui::{
        App, Bounds, Button, Context, IntoElement, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
        ParentElement, Pixels, Point, Render, SharedString, Styled as _, TitlebarOptions, Window,
        WindowBounds, WindowOptions, div, hsla, prelude::*, px, size,
    };
    use rgpui_platform::application;

    const PEN_WIDTH: Pixels = px(3.0);
    const HIGHLIGHTER_WIDTH: Pixels = px(16.0);
    const ERASER_RADIUS: Pixels = px(14.0);

    struct InkApp {
        /// 多图层墨迹文档。
        doc: InkDocument,
        /// 当前工具。
        tool: InkTool,
        /// 进行中的套索多边形。
        lasso: Vec<Point<Pixels>>,
        /// 橡皮是否局部擦除（false = 整笔擦除）。
        local_erase: bool,
        /// 套索选中的 `(图层, 笔画)`。
        selected: Vec<(usize, usize)>,
        /// 是否处于落笔状态。
        drawing: bool,
        /// 是否用合成压感绘制（演示逐段变宽）。
        pressure: bool,
        /// 本次笔画的起点（用来算合成压感）。
        stroke_origin: Option<Point<Pixels>>,
        /// 状态提示。
        status: SharedString,
    }

    impl InkApp {
        /// 创建示例应用。
        fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
            Self {
                doc: InkDocument::new(),
                tool: InkTool::Pen,
                local_erase: false,
                lasso: Vec::new(),
                selected: Vec::new(),
                drawing: false,
                pressure: false,
                stroke_origin: None,
                status: "拖拽绘制 · 切换工具试试橡皮与套索".into(),
            }
        }

        /// 当前工具对应的画笔样式。
        fn style(&self) -> InkStyle {
            match self.tool {
                InkTool::Highlighter => {
                    InkStyle::highlighter(hsla(0.13, 1.0, 0.5, 1.0), HIGHLIGHTER_WIDTH)
                }
                _ => InkStyle::pen(hsla(0.6, 0.75, 0.35, 1.0), PEN_WIDTH),
            }
        }

        /// 落笔。
        fn on_down(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
            self.drawing = true;
            self.selected.clear();
            self.stroke_origin = Some(position);
            match self.tool {
                InkTool::Pen | InkTool::Highlighter => {
                    let started = if self.pressure {
                        self.doc
                            .begin_stroke_with_pressure(position, 0.35, self.style())
                    } else {
                        self.doc.begin_stroke(position, self.style())
                    };
                    if !started {
                        self.status = "当前图层已隐藏或锁定，无法落笔".into();
                    }
                }
                InkTool::Eraser => {
                    if self.local_erase {
                        // 局部擦除：只剪掉圆内的采样点，笔画被切成两段。
                        let removed = self.doc.erase_local(position, ERASER_RADIUS);
                        self.status = format!("局部擦除 {removed} 个采样点").into();
                    } else {
                        let removed = self.doc.erase_at(position, ERASER_RADIUS);
                        self.status = format!("擦除 {removed} 条笔画").into();
                    }
                }
                InkTool::Lasso => self.lasso = vec![position],
            }
            cx.notify();
        }

        /// 合成本次采样的压感：沿笔画长度做正弦起伏，便于观察逐段变宽。
        fn synthetic_pressure(&self, position: Point<Pixels>) -> f32 {
            let Some(origin) = self.stroke_origin else {
                return 1.0;
            };
            let dx = (position.x - origin.x).as_f32();
            let dy = (position.y - origin.y).as_f32();
            let distance = (dx * dx + dy * dy).sqrt();
            0.25 + 0.75 * (0.5 + 0.5 * (distance / 45.0).sin())
        }

        /// 拖拽中。
        fn on_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
            if !self.drawing {
                return;
            }
            match self.tool {
                InkTool::Pen | InkTool::Highlighter => {
                    if self.pressure {
                        let pressure = self.synthetic_pressure(position);
                        self.doc.extend_stroke_with_pressure(position, pressure);
                    } else {
                        self.doc.extend_stroke(position);
                    }
                }
                InkTool::Eraser => {
                    self.doc.erase_at(position, ERASER_RADIUS);
                }
                InkTool::Lasso => self.lasso.push(position),
            }
            cx.notify();
        }

        /// 抬笔。
        fn on_up(&mut self, cx: &mut Context<Self>) {
            self.stroke_origin = None;
            if !self.drawing {
                return;
            }
            self.drawing = false;
            match self.tool {
                InkTool::Pen | InkTool::Highlighter => {
                    self.doc.end_stroke();
                }
                InkTool::Lasso => {
                    self.selected = self.doc.lasso(&self.lasso);
                    self.status = format!("套索选中 {} 条笔画", self.selected.len()).into();
                    self.lasso.clear();
                }
                InkTool::Eraser => {}
            }
            cx.notify();
        }

        /// 切换工具。
        fn set_tool(&mut self, tool: InkTool, cx: &mut Context<Self>) {
            self.tool = tool;
            self.status = format!("当前工具：{tool:?}").into();
            cx.notify();
        }

        /// 删除选中的笔画（跨图层，锁定 / 隐藏层跳过）。
        fn delete_selected(&mut self, cx: &mut Context<Self>) {
            let count = self.selected.len();
            self.doc.remove(&self.selected);
            self.selected.clear();
            self.status = format!("删除 {count} 条选中笔画").into();
            cx.notify();
        }

        /// 撤销激活图层最近一条笔画。
        fn undo(&mut self, cx: &mut Context<Self>) {
            self.selected.clear();
            let undone = self.doc.undo();
            self.status = if undone {
                "已撤销一条笔画".into()
            } else {
                "激活图层没有可撤销的笔画".into()
            };
            cx.notify();
        }

        /// 清空激活图层。
        fn clear(&mut self, cx: &mut Context<Self>) {
            self.doc.clear();
            self.selected.clear();
            self.status = "已清空激活图层".into();
            cx.notify();
        }

        /// 新建图层并激活。
        fn add_layer(&mut self, cx: &mut Context<Self>) {
            let name = format!("图层 {}", self.doc.layer_count() + 1);
            let index = self.doc.add_layer(name);
            self.status = format!("新建图层 #{}", index + 1).into();
            cx.notify();
        }

        /// 切换到下一图层。
        fn next_layer(&mut self, cx: &mut Context<Self>) {
            let next = (self.doc.active_index() + 1) % self.doc.layer_count();
            self.doc.set_active(next);
            self.status = format!("激活图层 #{}", next + 1).into();
            cx.notify();
        }

        /// 隐藏 / 显示激活图层。
        fn toggle_visible(&mut self, cx: &mut Context<Self>) {
            let index = self.doc.active_index();
            let visible = !self.doc.active().visible;
            self.doc.set_visible(index, visible);
            self.status = if visible {
                "显示激活图层".into()
            } else {
                "隐藏激活图层".into()
            };
            cx.notify();
        }

        /// 锁定 / 解锁激活图层。
        fn toggle_locked(&mut self, cx: &mut Context<Self>) {
            let index = self.doc.active_index();
            let locked = !self.doc.active().locked;
            self.doc.set_locked(index, locked);
            self.status = if locked {
                "锁定激活图层".into()
            } else {
                "解锁激活图层".into()
            };
            cx.notify();
        }
    }

    impl Render for InkApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let tool_button = |id: &'static str, label: &'static str, tool: InkTool| {
                Button::new(id).label(label).on_click(cx.listener(
                    move |this, _event, _window, cx| {
                        this.set_tool(tool, cx);
                    },
                ))
            };

            let active = self.doc.active_index();
            let layer_count = self.doc.layer_count();
            // 可见图层逐层绘制：套索只画在激活层，高亮按层过滤。
            let layer_elements: Vec<rgpui::AnyElement> = self
                .doc
                .visible_layers()
                .map(|(index, layer)| {
                    let highlight = self
                        .selected
                        .iter()
                        .filter(|(selected_layer, _)| *selected_layer == index)
                        .map(|(_, stroke)| *stroke)
                        .collect();
                    let lasso = if index == active {
                        self.lasso.clone()
                    } else {
                        Vec::new()
                    };
                    InkLayerElement::new(layer.layer.clone())
                        .opacity(layer.opacity)
                        .highlight(highlight)
                        .lasso(lasso)
                        // 每层把笔画暴露给辅助技术（画布合成子节点，播报点数 / 工具）。
                        .a11y(("ink-layer", index), layer.name.clone())
                        .into_any_element()
                })
                .collect();
            let active_layer = self.doc.active().clone();
            let active_strokes = active_layer.layer.strokes.len();
            let selected_count = self.selected.len();
            let selected_label = if self.selected.is_empty() {
                String::new()
            } else {
                format!(" · 选中 {selected_count}")
            };

            div()
                .relative()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                // 墨迹画布层：铺满窗口（坐标即窗口坐标），在最底层。
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .children(layer_elements)
                        .on_mouse_down(
                            rgpui::MouseButton::Left,
                            cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                this.on_down(ev.position, cx);
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                            this.on_move(ev.position, cx);
                        }))
                        .on_mouse_up(
                            rgpui::MouseButton::Left,
                            cx.listener(|this, _ev: &MouseUpEvent, _window, cx| this.on_up(cx)),
                        ),
                )
                // 工具条：绘制在画布之上。
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_2()
                                .child(tool_button("tool-pen", "钢笔", InkTool::Pen))
                                .child(tool_button(
                                    "tool-highlighter",
                                    "荧光笔",
                                    InkTool::Highlighter,
                                ))
                                .child(tool_button("tool-eraser", "橡皮", InkTool::Eraser))
                                .child(tool_button("tool-lasso", "套索", InkTool::Lasso))
                                .child(
                                    Button::new("pressure")
                                        .label(if self.pressure {
                                            "压感笔：开"
                                        } else {
                                            "压感笔：关"
                                        })
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.pressure = !this.pressure;
                                            this.status = if this.pressure {
                                                "压感笔：笔画按合成压感逐段变宽".into()
                                            } else {
                                                "压感笔：固定宽度".into()
                                            };
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("local-erase")
                                        .label(if self.local_erase {
                                            "橡皮：局部"
                                        } else {
                                            "橡皮：整笔"
                                        })
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.local_erase = !this.local_erase;
                                            cx.notify();
                                        })),
                                )
                                .child(Button::new("undo").label("撤销").on_click(cx.listener(
                                    |this, _event, _window, cx| {
                                        this.undo(cx);
                                    },
                                )))
                                .child(Button::new("delete-selected").label("删除选中").on_click(
                                    cx.listener(|this, _event, _window, cx| {
                                        this.delete_selected(cx);
                                    }),
                                ))
                                .child(Button::new("clear").label("清空").on_click(
                                    cx.listener(|this, _event, _window, cx| this.clear(cx)),
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_2()
                                .child(Button::new("add-layer").label("新建图层").on_click(
                                    cx.listener(|this, _event, _window, cx| this.add_layer(cx)),
                                ))
                                .child(Button::new("next-layer").label("切换图层").on_click(
                                    cx.listener(|this, _event, _window, cx| this.next_layer(cx)),
                                ))
                                .child(Button::new("toggle-visible").label("隐藏/显示").on_click(
                                    cx.listener(|this, _event, _window, cx| {
                                        this.toggle_visible(cx);
                                    }),
                                ))
                                .child(Button::new("toggle-locked").label("锁定/解锁").on_click(
                                    cx.listener(|this, _event, _window, cx| {
                                        this.toggle_locked(cx);
                                    }),
                                )),
                        )
                        .child(div().text_sm().child(self.status.clone()))
                        .child(div().text_xs().child(format!(
                            "图层 {}/{}（{}）· 激活层笔画 {} · 可见层 {}{}",
                            active + 1,
                            layer_count,
                            if active_layer.visible {
                                "可见"
                            } else {
                                "隐藏"
                            },
                            active_strokes,
                            self.doc.visible_layers().count(),
                            selected_label,
                        ))),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(760.), px(560.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Ink Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| InkApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| InkApp::new(window, cx)).into()
    }
}

mod sorter {
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

    struct Slide {
        /// 标题。
        title: SharedString,
        /// 封面底色。
        color: Hsla,
    }

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
                    SorterEvent::ReorderMany { from, to } => {
                        state.update(cx, |state, cx| {
                            let slides = &mut state.delegate_mut().slides;
                            // 先按原索引抽出整组，再整组插入到落位索引
                            let len = slides.len();
                            let moved: Vec<_> = from
                                .iter()
                                .rev()
                                .filter(|index| **index < len)
                                .map(|index| slides.remove(*index))
                                .collect();
                            for (offset, slide) in moved.into_iter().rev().enumerate() {
                                slides.insert(to + offset, slide);
                            }
                            cx.notify();
                        });
                        this.status =
                            format!("整组移动 {} 页到第 {} 位", from.len(), to + 1).into();
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
                            Button::new("add").label("新建").on_click(
                                cx.listener(|this, _event, _window, cx| this.add_slide(cx)),
                            ),
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

    pub fn run_example() {
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

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| SorterApp::new(window, cx)).into()
    }
}

mod timeline {
    //! 动画时间线示例
    //!
    //! 演示 `anim_timeline`：多轨关键帧时间线（标尺刻度 + 值曲线 + 关键帧方块 + 播放头）。
    //! 拖拽标尺移动播放头，拖拽关键帧改时间、上下拖拽改值，双击轨道空白处新增关键帧
    //! （值取当前插值），`Delete` 删除选中关键帧，`Ctrl/Cmd + 滚轮` 缩放、
    //! `Shift + 滚轮` 纵向滚动轨道、滚轮平移时间轴，点轨道名前的箭头折叠 / 展开轨道。
    //! 三轨的值实时驱动右侧预览方块（位置 / 缩放 / 透明度），播放头播放时按 `1/60s` 步进推进。
    //!
    //! 运行：`cargo run -p showcase --bin timeline`

    use std::time::Duration;

    use rgpui::anim_timeline::{
        Interpolation, Keyframe, Timeline, TimelineEvent, TimelineState, TimelineView, Track,
    };
    use rgpui::{
        AnyElement, App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
        prelude::*, px, size, v_flex,
    };
    use rgpui_platform::application;

    const FRAME_STEP: f32 = 1.0 / 60.0;

    fn demo_timeline() -> Timeline {
        Timeline::from_tracks(vec![
            Track::with_keyframes(
                "位置 X",
                vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 240.0)],
            ),
            Track::with_keyframes(
                "缩放",
                vec![Keyframe::new(0.0, 0.6), Keyframe::new(2.0, 1.4)],
            )
            .with_interpolation(Interpolation::Smooth),
            Track::with_keyframes(
                "透明度",
                vec![
                    Keyframe::new(0.0, 0.2),
                    Keyframe::new(1.0, 1.0),
                    Keyframe::new(2.0, 0.6),
                ],
            ),
            // 后三条只用来把轨道区撑到需要纵向滚动，预览方块不读取。
            Track::with_keyframes(
                "旋转",
                vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 90.0)],
            ),
            Track::with_keyframes(
                "模糊",
                vec![Keyframe::new(0.0, 0.0), Keyframe::new(2.0, 8.0)],
            ),
            Track::new("颜色"),
        ])
    }

    struct TimelineApp {
        /// 时间线状态。
        state: Entity<TimelineState>,
        /// 是否播放中（与时间线状态同步）。
        playing: bool,
        /// 状态栏文案。
        status: SharedString,
    }

    impl TimelineApp {
        /// 创建示例应用并订阅时间线事件。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let state = cx.new(|cx| TimelineState::new(demo_timeline(), cx));

            cx.subscribe_in(
                &state,
                window,
                |this: &mut Self, state, event: &TimelineEvent, _window, cx| match event {
                    TimelineEvent::PlaybackChanged(playing) => {
                        this.playing = *playing;
                        this.status = if *playing {
                            "播放中…".into()
                        } else {
                            "已暂停".into()
                        };
                    }
                    TimelineEvent::KeyframeRequested { track, time } => {
                        state.update(cx, |state, cx| {
                            if let Some(model) = state.timeline_mut().track_mut(*track) {
                                let value = model.value_at(*time);
                                model.set_keyframe(*time, value);
                            }
                            cx.notify();
                        });
                        this.status = format!("在第 {:.2} 秒新增关键帧", time).into();
                    }
                    TimelineEvent::KeyframeMoved {
                        track, time, value, ..
                    } => {
                        this.status = format!(
                            "轨道 {} 的关键帧 → {:.2} 秒 / 值 {:.2}",
                            track + 1,
                            time,
                            value
                        )
                        .into();
                    }
                    TimelineEvent::KeyframeRemoved { track, .. } => {
                        this.status = format!("删除轨道 {} 的一个关键帧", track + 1).into();
                    }
                    TimelineEvent::TimeChanged(time) => {
                        this.status = format!("播放头 {:.2} 秒", time).into();
                    }
                    TimelineEvent::SelectionChanged { .. } => {}
                    TimelineEvent::TrackMoved { from, to } => {
                        this.status = format!("轨道 {} 移到第 {} 位", from + 1, to + 1).into();
                    }
                    TimelineEvent::TrackCollapsed { index, collapsed } => {
                        this.status = if *collapsed {
                            format!("轨道 {} 已折叠", index + 1)
                        } else {
                            format!("轨道 {} 已展开", index + 1)
                        }
                        .into();
                    }
                    TimelineEvent::TrackInterpolation {
                        index,
                        interpolation,
                    } => {
                        this.status =
                            format!("轨道 {} 的插值改为「{}」", index + 1, interpolation.label())
                                .into();
                    }
                },
            )
            .detach();

            Self {
            state,
            playing: false,
            status: "拖拽标尺移动播放头 · 拖拽关键帧改时间 / 改值 · Ctrl+点击多选关键帧 · 双击轨道空白新增".into(),
        }
        }

        /// 开始播放（后台逐帧推进播放头）。
        fn play(&mut self, cx: &mut Context<Self>) {
            if self.playing {
                return;
            }
            self.playing = true;
            self.state.update(cx, |state, cx| state.play(cx));

            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    let continues = this.update(cx, |this, cx| {
                        if !this.playing {
                            return false;
                        }
                        this.state
                            .update(cx, |state, cx| state.advance(FRAME_STEP, cx));
                        true
                    });
                    if !matches!(continues, Ok(true)) {
                        break;
                    }
                }
            })
            .detach();
        }

        /// 暂停播放。
        fn pause(&mut self, cx: &mut Context<Self>) {
            self.playing = false;
            self.state.update(cx, |state, cx| state.pause(cx));
        }

        /// 回到开头。
        fn rewind(&mut self, cx: &mut Context<Self>) {
            self.state.update(cx, |state, cx| state.set_time(0.0, cx));
            self.status = "回到开头".into();
        }

        /// 在播放头处给第一条轨道新增关键帧。
        fn add_keyframe(&mut self, cx: &mut Context<Self>) {
            let (time, index) = self.state.update(cx, |state, cx| {
                let time = state.time();
                let Some(model) = state.timeline_mut().track_mut(0) else {
                    return (0.0, None);
                };
                let value = model.value_at(time);
                model.set_keyframe(time, value);
                let index = model.keyframe_index_at(time, 1e-3);
                cx.notify();
                (time, index)
            });
            if let Some(index) = index {
                self.state
                    .update(cx, |state, cx| state.select_keyframe(0, index, cx));
            }
            self.status = format!("在 {time:.2} 秒新增关键帧").into();
        }

        /// 删除选中的关键帧。
        fn remove_keyframe(&mut self, cx: &mut Context<Self>) {
            let selected = self.state.read(cx).selection().1.is_some();
            self.state
                .update(cx, |state, cx| state.remove_selected_keyframe(cx));
            self.status = if selected {
                "删除选中的关键帧".into()
            } else {
                "没有选中关键帧".into()
            };
        }

        /// 时间线总时长。
        fn duration(&self, cx: &Context<Self>) -> f32 {
            self.state.read(cx).duration()
        }

        /// 上移 / 下移选中的轨道（未选中时移动第一条）。
        fn move_track(&mut self, delta: isize, cx: &mut Context<Self>) {
            let (from, count) = {
                let state = self.state.read(cx);
                (state.selection().0.unwrap_or(0), state.timeline().len())
            };
            let target = from as isize + delta;
            if target < 0 || target >= count as isize {
                self.status = "轨道已在边界".into();
                return;
            }
            self.state
                .update(cx, |state, cx| state.move_track(from, target as usize, cx));
        }

        /// 折叠 / 展开选中的轨道（未选中时处理第一条）。
        fn toggle_collapse(&mut self, cx: &mut Context<Self>) {
            let index = self.state.read(cx).selection().0.unwrap_or(0);
            self.state
                .update(cx, |state, cx| state.toggle_track_collapsed(index, cx));
        }

        /// 播放头处三条轨道的值（位置 / 缩放 / 透明度）。
        fn values(&self, cx: &Context<Self>) -> (f32, f32, f32) {
            let state = self.state.read(cx);
            let values = state.timeline().values_at(state.time());
            (
                values.first().copied().unwrap_or(0.0),
                values.get(1).copied().unwrap_or(1.0),
                values.get(2).copied().unwrap_or(1.0),
            )
        }
    }

    fn interpolation_buttons(
        cx: &mut Context<TimelineApp>,
        state: Entity<TimelineState>,
    ) -> Vec<AnyElement> {
        let selected = state.read(cx).selection().0;
        let mut out = Vec::new();
        for (index, interpolation) in [
            Interpolation::Linear,
            Interpolation::Smooth,
            Interpolation::EaseIn,
            Interpolation::EaseOut,
            Interpolation::EaseInOut,
            Interpolation::Hold,
        ]
        .into_iter()
        .enumerate()
        {
            let state = state.clone();
            let current = selected.and_then(|track| {
                state
                    .read(cx)
                    .timeline()
                    .track(track)
                    .map(|track| track.interpolation)
            });
            let active = current == Some(interpolation);
            let button = Button::new(format!("interp-{index}"))
                .label(format!("插值：{}", interpolation.label()))
                .on_click(move |_event, _window, cx| {
                    state.update(cx, |state, cx| {
                        if let Some(track) = state.selection().0 {
                            state.set_track_interpolation(track, interpolation, cx);
                        }
                    });
                });
            out.push(
                div()
                    .child(button)
                    .when(active, |this| this.text_color(cx.theme().primary))
                    .into_any_element(),
            );
        }
        out
    }
    impl Render for TimelineApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let (x, scale, opacity) = self.values(cx);
            let duration = self.duration(cx);
            let time = self.state.read(cx).time();
            let snapping = self.state.read(cx).is_snapping();
            let selected = self.state.read(cx).selection().1.is_some();

            let play_button = if self.playing {
                Button::new("pause")
                    .label("暂停")
                    .on_click(cx.listener(|this, _event, _window, cx| this.pause(cx)))
            } else {
                Button::new("play")
                    .label("播放")
                    .on_click(cx.listener(|this, _event, _window, cx| this.play(cx)))
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
                        .child(play_button)
                        .child(
                            Button::new("rewind")
                                .label("回到开头")
                                .on_click(cx.listener(|this, _event, _window, cx| this.rewind(cx))),
                        )
                        .child(Button::new("add-keyframe").label("加关键帧").on_click(
                            cx.listener(|this, _event, _window, cx| this.add_keyframe(cx)),
                        ))
                        .child(Button::new("remove-keyframe").label("删关键帧").on_click(
                            cx.listener(|this, _event, _window, cx| this.remove_keyframe(cx)),
                        ))
                        .child(Button::new("zoom-in").label("放大").on_click(cx.listener(
                            |this, _event, _window, cx| {
                                let state = this.state.clone();
                                state.update(cx, |state, cx| state.zoom_by(1.25, cx));
                            },
                        )))
                        .child(Button::new("zoom-out").label("缩小").on_click(cx.listener(
                            |this, _event, _window, cx| {
                                let state = this.state.clone();
                                state.update(cx, |state, cx| state.zoom_by(0.8, cx));
                            },
                        )))
                        .child(Button::new("track-up").label("轨道上移").on_click(
                            cx.listener(|this, _event, _window, cx| this.move_track(-1, cx)),
                        ))
                        .child(Button::new("track-down").label("轨道下移").on_click(
                            cx.listener(|this, _event, _window, cx| this.move_track(1, cx)),
                        ))
                        .child(Button::new("track-fold").label("折叠轨道").on_click(
                            cx.listener(|this, _event, _window, cx| this.toggle_collapse(cx)),
                        ))
                        .children(interpolation_buttons(cx, self.state.clone()))
                        .child(
                            Button::new("toggle-snap")
                                .label(if snapping {
                                    "吸附：开"
                                } else {
                                    "吸附：关"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    let state = this.state.clone();
                                    let next = if state.read(cx).is_snapping() {
                                        0.0
                                    } else {
                                        30.0
                                    };
                                    state.update(cx, |state, cx| state.set_fps(next, cx));
                                })),
                        )
                        .child(div().text_sm().child(self.status.clone())),
                )
                .child(
                    div()
                        .h(px(140.0))
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(TimelineView::new(&self.state)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .p_4()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .child(format!("播放头 {time:.2}s / 总时长 {duration:.2}s")),
                        )
                        .child(div().text_xs().child(format!(
                            "位置 X {x:.1}px · 缩放 {scale:.2}× · 透明度 {opacity:.2}"
                        )))
                        .child(
                            div()
                                .relative()
                                .h(px(160.0))
                                .w_full()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .overflow_hidden()
                                // 预览方块：位置 / 缩放 / 透明度由时间线驱动。
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(40.0))
                                        .left(px(16.0 + x))
                                        .w(px(120.0 * scale))
                                        .h(px(80.0 * scale))
                                        .rounded_md()
                                        .bg(hsla(0.58, 0.6, 0.5, 1.0))
                                        .opacity(opacity),
                                ),
                        )
                        .child(div().text_xs().child(if selected {
                            "已选中关键帧：Delete 删除，水平拖拽改时间、上下拖拽改值"
                        } else {
                            "未选中关键帧"
                        })),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1000.), px(620.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("动画时间线示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TimelineApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| TimelineApp::new(window, cx)).into()
    }
}

mod color_picker {
    //! 取色器增强示例
    //!
    //! 演示 [`ColorPickerState`] 的主题色板 / 最近颜色快捷色块，以及可独立复用的
    //! [`ColorPalette`]：
    //! - 取色器（三滑块 + hex + 主题色板 + 最近颜色）：点击色块即应用并记录；
    //! - hex 输入框回车把当前颜色记入最近颜色；
    //! - 独立色板：把命名颜色色阶（`ColorName::all()` 的 500 色阶）铺成一格一格，
    //!   点击直接应用到取色器；
    //! - 「记录当前颜色 / 清空最近」按钮演示程序化维护最近颜色。
    //!
    //! 启用 `screen-capture` feature（showcase 默认开启）时，动作行多出「吸管取色」：
    //! 调 [`ColorPickerState::begin_eyedropper`]，从屏幕首帧取色并写回取色器。
    //!
    //! 运行：`cargo run -p showcase --bin color_picker`

    use rgpui::{
        App, Bounds, Button, Context, Entity, Hsla, IntoElement, ParentElement, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions,
        components::{ColorPalette, ColorPickerState, color_name_palette},
        div, hsla,
        prelude::*,
        px, size,
    };
    use rgpui_platform::application;

    struct ColorPickerApp {
        /// 指向自身的句柄。
        handle: Entity<Self>,
        /// 取色器（含主题色板 / 最近颜色）。
        picker: Entity<ColorPickerState>,
        /// 当前颜色。
        color: Hsla,
        /// 状态提示。
        status: SharedString,
    }

    impl ColorPickerApp {
        /// 创建示例应用。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let handle = cx.entity();
            let initial = hsla(210.0 / 360.0, 0.6, 0.5, 1.0);
            let picker = cx.new(|cx| {
                ColorPickerState::new(window, cx).color(initial).on_change({
                    let handle = handle.clone();
                    move |color, _window, cx| {
                        handle.update(cx, |this, cx| {
                            this.color = color;
                            cx.notify();
                        });
                    }
                })
            });
            Self {
                handle,
                picker,
                color: initial,
                status: "点击色板色块、拖动滑块或输入 hex（回车提交）".into(),
            }
        }

        /// 把取色器当前颜色记入最近颜色。
        fn remember(&mut self, cx: &mut Context<Self>) {
            let color = self.picker.read(cx).color_value();
            let changed = self
                .picker
                .update(cx, |picker, cx| picker.push_recent(color, cx));
            self.status = if changed {
                "已记入最近颜色".into()
            } else {
                "该颜色已排在最近颜色首位".into()
            };
            cx.notify();
        }

        /// 清空最近颜色。
        fn clear(&mut self, cx: &mut Context<Self>) {
            self.picker
                .update(cx, |picker, cx| picker.clear_recents(cx));
            self.status = "已清空最近颜色".into();
            cx.notify();
        }
    }

    impl Render for ColorPickerApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let handle = self.handle.clone();

            // 独立色板：命名颜色 500 色阶，点击直接应用到取色器（复用 `ColorPalette`）。
            let picker_for_scale = self.picker.clone();
            let scale_palette = ColorPalette::new("scale-500")
                .colors(color_name_palette(500))
                .selected(Some(self.color))
                .swatch_size(20.0)
                .gap(6.0)
                .on_select(move |color, window, cx| {
                    picker_for_scale.update(cx, |picker, cx| picker.set_color(color, window, cx));
                });

            let hex = hex_string(self.color);

            // 动作行：吸管按钮只在 `screen-capture` feature 下编译（底层走平台屏幕捕获）。
            let action_row = {
                let row = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("remember").label("记录当前颜色").on_click({
                        let handle = handle.clone();
                        move |_event, _window, cx| handle.update(cx, |this, cx| this.remember(cx))
                    }))
                    .child(Button::new("clear").label("清空最近").on_click({
                        move |_event, _window, cx| handle.update(cx, |this, cx| this.clear(cx))
                    }));
                #[cfg(feature = "screen-capture")]
                let row = {
                    let picker = self.picker.clone();
                    row.child(Button::new("eyedropper").label("吸管取色").on_click(
                        move |_event, _window, cx| {
                            picker.update(cx, |picker, cx| picker.begin_eyedropper(cx));
                        },
                    ))
                };
                row
            };

            div()
                .flex()
                .flex_row()
                .gap_6()
                .p_6()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_xl().child("取色器增强示例"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.status.clone()),
                        )
                        .child(self.picker.clone())
                        .child(action_row),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .flex_1()
                        .child(div().text_lg().child("预览"))
                        .child(
                            div()
                                .p_4()
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(self.color)
                                .child(
                                    div()
                                        .text_color(contrast_foreground(self.color))
                                        .child(format!("当前颜色 {hex}")),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("命名色阶色板（500）—— 点击直接应用"),
                        )
                        .child(scale_palette),
                )
        }
    }

    fn hex_string(color: Hsla) -> String {
        let rgba = color.to_rgb();
        let channel = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            channel(rgba.r),
            channel(rgba.g),
            channel(rgba.b)
        )
    }

    fn contrast_foreground(color: Hsla) -> Hsla {
        if color.l > 0.55 {
            hsla(0.0, 0.0, 0.05, 1.0)
        } else {
            hsla(0.0, 0.0, 1.0, 1.0)
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(880.), px(620.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("取色器示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ColorPickerApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ColorPickerApp::new(window, cx)).into()
    }
}

mod mdi {
    //! 多文档界面（MDI）示例
    //!
    //! 演示 `mdi`：一块 MDI 区域内并列多个子文档窗口。每个窗口带标题栏（最小化 / 最大化 /
    //! 关闭）与八向缩放边框；拖拽标题栏移动、拖拽边框缩放、双击标题栏切换最大化、
    //! 最小化的窗口缩为底部图标条。工具栏提供新建 / 层叠 / 平铺 / 页签 / 全部最小化 / 关闭全部，
    //! 激活窗口内按 `Ctrl+Tab`（或 `Ctrl+F6`）循环切换、`Ctrl+F4` 关闭、`Ctrl+M` 最小化。
    //! 窗口正文里放了一个按钮，用来验证「正文可交互 + 点击自动激活窗口」。
    //!
    //! 运行：`cargo run -p showcase --bin mdi`

    use std::collections::HashMap;

    use rgpui::mdi::{MdiArea, MdiEvent, MdiLayout, MdiState, MdiWindowFlags};
    use rgpui::{
        App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, hsla,
        prelude::*, px, size, v_flex,
    };
    use rgpui_platform::application;

    fn open_document(app: &mut MdiApp, title: String, cx: &mut Context<MdiApp>) {
        app.mdi.update(cx, |state, cx| state.open(title, cx));
    }

    struct MdiApp {
        /// MDI 状态。
        mdi: Entity<MdiState>,
        /// 各窗口正文里的点击次数（演示正文可交互）。
        clicks: HashMap<u64, usize>,
        /// 下一个文档序号。
        next_index: usize,
        /// 状态栏文案。
        status: SharedString,
    }

    impl MdiApp {
        /// 创建示例应用并订阅 MDI 事件。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            // 拖动松手时吸附到工作区/邻窗边缘（6px 内）
            let mdi = cx.new(|cx| MdiState::new(cx).with_snap(px(6.0)));

            // MDI 元素只负责画与转发事件，几何变化要靠宿主观察状态来触发重绘。
            cx.observe(&mdi, |_this, _mdi, cx| cx.notify()).detach();

            cx.subscribe_in(
                &mdi,
                window,
                |this: &mut Self, _mdi, event: &MdiEvent, _window, cx| {
                    this.status = match event {
                        MdiEvent::Opened(id) => format!("打开窗口 {id}").into(),
                        MdiEvent::Activated(id) => format!("激活窗口 {id}").into(),
                        MdiEvent::Closed(id) => format!("关闭窗口 {id}").into(),
                        MdiEvent::StateChanged(id) => format!("窗口 {id} 状态变化").into(),
                    };
                    cx.notify();
                },
            )
            .detach();

            let mut app = Self {
                mdi,
                clicks: HashMap::new(),
                next_index: 0,
                status: "拖拽标题栏移动（松手吸附边缘）· 拖拽边框缩放 · 双击标题栏最大化".into(),
            };
            // 初始铺开三个文档，方便一眼看到层叠效果。
            for name in ["季度报告", "预算表", "会议纪要"] {
                app.new_document(name.into(), cx);
            }
            app.mdi
                .update(cx, |state, cx| state.arrange(MdiLayout::Cascade, cx));
            app
        }

        /// 新建一个文档窗口。
        fn new_document(&mut self, name: String, cx: &mut Context<Self>) {
            self.next_index += 1;
            let title = format!("{name} ({})", self.next_index);
            open_document(self, title, cx);
        }

        /// 新建一个不可关闭 / 不可最小化的只读窗口。
        fn new_readonly(&mut self, cx: &mut Context<Self>) {
            self.next_index += 1;
            let title = format!("只读视图 ({})", self.next_index);
            self.mdi.update(cx, |state, cx| {
                state.open_with(
                    title,
                    MdiWindowFlags {
                        closable: false,
                        minimizable: false,
                        maximizable: true,
                    },
                    cx,
                );
            });
        }

        /// 按给定方式排布窗口。
        fn arrange(&mut self, layout: MdiLayout, cx: &mut Context<Self>) {
            self.mdi.update(cx, |state, cx| state.arrange(layout, cx));
            self.status = match layout {
                MdiLayout::Cascade => "层叠排布".into(),
                MdiLayout::TileHorizontal => "水平平铺".into(),
                MdiLayout::TileVertical => "垂直平铺".into(),
                MdiLayout::Tabbed => "页签式（点击标签切换）".into(),
            };
        }

        /// 最小化全部窗口。
        fn minimize_all(&mut self, cx: &mut Context<Self>) {
            self.mdi.update(cx, |state, cx| {
                for window in state
                    .windows()
                    .iter()
                    .map(|window| window.id)
                    .collect::<Vec<_>>()
                {
                    state.minimize(window, cx);
                }
            });
            self.status = "全部最小化（点击底部图标还原）".into();
        }

        /// 关闭全部窗口。
        fn close_all(&mut self, cx: &mut Context<Self>) {
            self.mdi.update(cx, |state, cx| state.close_all(cx));
            self.clicks.clear();
            self.status = "关闭全部窗口".into();
        }
    }

    impl Render for MdiApp {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let (count, active) = {
                let state = self.mdi.read(cx);
                (state.len(), state.active())
            };
            let status = self.status.clone();

            // 正文回调：每个窗口按 id 现渲染（这里放一个可点击按钮，验证正文可交互）。
            let this = cx.entity();
            let body = move |id: u64, _window: &mut Window, cx: &mut App| {
                let this = this.clone();
                let count = this.read(cx).clicks.get(&id).copied().unwrap_or(0);
                let (hue, tint) = (0.58 + 0.13 * (id % 4) as f32, 0.94);
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .size_full()
                    .p_3()
                    .bg(hsla(hue, 0.25, tint, 1.0))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("文档 {id} 的正文（点击计数 {count}）")),
                    )
                    .child(Button::new(("bump", id)).label("点我 +1").on_click(
                        move |_event, _window, cx| {
                            this.update(cx, |this, cx| {
                                *this.clicks.entry(id).or_default() += 1;
                                this.status = format!("文档 {id} 正文被点击").into();
                                cx.notify();
                            });
                        },
                    ))
                    .child(
                        div()
                            .text_xs()
                            .child("正文内的控件可以正常交互，点击正文会顺带激活所在窗口。"),
                    )
                    .into_any_element()
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
                        .child(Button::new("new").label("新建文档").on_click(cx.listener(
                            |this, _event, _window, cx| {
                                this.new_document("新文档".into(), cx);
                            },
                        )))
                        .child(
                            Button::new("new-readonly")
                                .label("只读视图")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.new_readonly(cx);
                                })),
                        )
                        .child(Button::new("cascade").label("层叠").on_click(cx.listener(
                            |this, _event, _window, cx| this.arrange(MdiLayout::Cascade, cx),
                        )))
                        .child(
                            Button::new("tile-h")
                                .label("水平平铺")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.arrange(MdiLayout::TileHorizontal, cx)
                                })),
                        )
                        .child(
                            Button::new("tile-v")
                                .label("垂直平铺")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.arrange(MdiLayout::TileVertical, cx)
                                })),
                        )
                        .child(Button::new("tabbed").label("页签").on_click(cx.listener(
                            |this, _event, _window, cx| this.arrange(MdiLayout::Tabbed, cx),
                        )))
                        .child(Button::new("minimize-all").label("全部最小化").on_click(
                            cx.listener(|this, _event, _window, cx| this.minimize_all(cx)),
                        ))
                        .child(
                            Button::new("close-all").label("关闭全部").on_click(
                                cx.listener(|this, _event, _window, cx| this.close_all(cx)),
                            ),
                        )
                        .child(div().text_sm().child(format!(
                                "窗口 {count} 个 · 激活 {} · {status}",
                                active
                                    .map(|id| format!("#{id}"))
                                    .unwrap_or_else(|| "无".to_string())
                            ))),
                )
                .child(
                    div()
                        .flex_1()
                        .p_2()
                        .child(MdiArea::new(&self.mdi).body(body)),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1080.), px(720.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("MDI Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| MdiApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| MdiApp::new(window, cx)).into()
    }
}

mod task_pane {
    //! 任务窗格示例
    //!
    //! 演示 `task_pane`：右侧停靠的任务窗格（标题栏 + 正文 + 内缘拖拽把手 + 关闭按钮）。
    //! 拖动内缘把手改宽，`✕` 关闭，工具栏可切换停靠侧 / 浮动 / 显隐；标题栏是「属性 / 大纲」
    //! 页签组（点击页签或 `Ctrl+Tab` 切换，正文随之换页）；打开「自动隐藏」后鼠标移出收起成
    //! 竖排标题轨道、移入展开。浮动态变成一块可自由拖动标题栏、八向缩放的窗格（复用 MDI 的
    //! 缩放几何）。窗格正文实时读取状态，展示宽度 / 停靠侧 / 浮动状态。
    //!
    //! 运行：`cargo run -p showcase --bin task_pane`

    use rgpui::task_pane::{TaskPane, TaskPaneEvent, TaskPaneSide, TaskPaneState};
    use rgpui::{
        AnyElement, App, Bounds, Button, Context, Entity, IntoElement, ParentElement as _, Render,
        SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, size, v_flex,
    };
    use rgpui_platform::application;

    struct TaskPaneApp {
        /// 窗格状态。
        pane: Entity<TaskPaneState>,
        /// 状态栏文案。
        status: SharedString,
    }

    impl TaskPaneApp {
        /// 创建示例应用并观察窗格状态。
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let pane = cx.new(|cx| {
                TaskPaneState::new(cx)
                    .with_side(TaskPaneSide::Right)
                    .with_width(px(300.0))
                    .with_tabs(["属性", "大纲"])
            });

            // 窗格元素只负责画与转发事件，几何变化要靠宿主观察状态来触发重绘。
            cx.observe(&pane, |_this, _pane, cx| cx.notify()).detach();
            cx.subscribe_in(
                &pane,
                window,
                |this: &mut Self, _pane, event: &TaskPaneEvent, _window, cx| {
                    this.status = match event {
                        TaskPaneEvent::VisibilityChanged(true) => "显示窗格".into(),
                        TaskPaneEvent::VisibilityChanged(false) => "隐藏窗格".into(),
                        TaskPaneEvent::SideChanged(TaskPaneSide::Left) => "停靠到左侧".into(),
                        TaskPaneEvent::SideChanged(TaskPaneSide::Right) => "停靠到右侧".into(),
                        TaskPaneEvent::WidthChanged(width) => format!("窗格宽 {width:.0}").into(),
                        TaskPaneEvent::FloatingChanged(true) => "切到浮动态".into(),
                        TaskPaneEvent::FloatingChanged(false) => "回到停靠态".into(),
                        TaskPaneEvent::Collapsed(true) => "窗格收起（悬停展开）".into(),
                        TaskPaneEvent::Collapsed(false) => "窗格展开".into(),
                        TaskPaneEvent::TabSelected(index) => format!(
                            "切到「{}」页签",
                            ["属性", "大纲"].get(*index).copied().unwrap_or("?")
                        )
                        .into(),
                    };
                    cx.notify();
                },
            )
            .detach();

            Self {
            pane,
            status:
                "拖动内缘把手改宽 · 把把手拖过窗口中线可换边停靠 · ✕ 关闭 · 点标题栏页签切换正文"
                    .into(),
        }
        }

        /// 切换浮动 / 停靠（浮动时贴到窗口右下角内侧）。
        fn toggle_float(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            let parent = Bounds::new(rgpui::point(px(0.0), px(0.0)), window.viewport_size());
            self.pane
                .update(cx, |pane, cx| pane.toggle_float(parent, cx));
        }

        /// 相对当前宽度加宽 / 收窄。
        fn resize_by(&mut self, delta: f32, cx: &mut Context<Self>) {
            self.pane.update(cx, |pane, cx| {
                let width = pane.width() + px(delta);
                pane.set_width(width, cx);
            });
        }
    }

    impl Render for TaskPaneApp {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 父容器矩形 = 窗口视口：把手拖过中线即可换边停靠。
            let parent = Bounds::new(rgpui::point(px(0.0), px(0.0)), window.viewport_size());
            self.pane
                .update(cx, |pane, cx| pane.set_parent_bounds(parent, cx));
            let (visible, side, width, floating, closable, auto_hide, collapsed) =
                self.pane.read_with(cx, |pane, _| {
                    (
                        pane.is_visible(),
                        pane.side(),
                        pane.width(),
                        pane.is_floating(),
                        pane.is_closable(),
                        pane.is_auto_hide(),
                        pane.is_collapsed(),
                    )
                });
            let status = self.status.clone();

            // 正文：按当前页签渲染属性行或大纲。
            let pane_for_body = self.pane.clone();
            let body = move |_window: &mut Window, cx: &mut App| -> AnyElement {
                let (width, side, floating, active) = pane_for_body.read_with(cx, |pane, _| {
                    (
                        pane.width(),
                        pane.side(),
                        pane.is_floating(),
                        pane.active_tab(),
                    )
                });
                let theme = cx.theme();
                let muted = theme.muted_foreground;
                let foreground = theme.foreground;
                let border = theme.border;
                let row = move |label: &'static str, value: String| {
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .py_1()
                        .border_b_1()
                        .border_color(border)
                        .text_sm()
                        .child(div().text_color(muted).child(label))
                        .child(div().text_color(foreground).child(value))
                };
                let container = v_flex().gap_1().p_3();
                if active == 1 {
                    return container
                        .child(div().text_xs().text_color(muted).child("文档大纲"))
                        .children(
                            ["一、概述", "二、页面设置", "三、停靠与浮动", "四、自动隐藏"].map(
                                |label| {
                                    div()
                                        .py_1()
                                        .border_b_1()
                                        .border_color(border)
                                        .text_sm()
                                        .text_color(foreground)
                                        .child(label)
                                },
                            ),
                        )
                        .child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(muted)
                                .child("点击标题栏页签或按 Ctrl+Tab 切换。"),
                        )
                        .into_any_element();
                }
                container
                    .child(div().text_xs().text_color(muted).child("页面设置"))
                    .child(row("宽度", format!("{width:.0}")))
                    .child(row(
                        "停靠侧",
                        if floating {
                            "浮动".to_string()
                        } else if side == TaskPaneSide::Left {
                            "左侧".to_string()
                        } else {
                            "右侧".to_string()
                        },
                    ))
                    .child(row("纸张", "A4 · 纵向".into()))
                    .child(row("页边距", "上下 2.54 · 左右 3.17".into()))
                    .child(
                        div()
                            .pt_2()
                            .text_xs()
                            .text_color(muted)
                            .child("属性行随窗格状态实时刷新；拖动内缘把手即可改宽。"),
                    )
                    .into_any_element()
            };

            // 主内容区。
            let main = div().flex_1().h_full().p_4().child(
            v_flex()
                .gap_2()
                .size_full()
                .child(div().text_base().child("文档正文"))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("这里是被停靠窗格挤出的可用区域。"),
                )
                .child(
                    div()
                        .flex_1()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .p_3()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("拖动窗格内缘的细条改宽 · 点标题栏页签切换属性 / 大纲 · 工具栏可切到浮动态"),
                ),
        );

            let pane = TaskPane::new(&self.pane)
                .title("属性")
                .body(body)
                .into_any_element();

            // 容器是浮动态的定位祖先。
            let row = div()
                .relative()
                .flex()
                .flex_row()
                .flex_1()
                .overflow_hidden()
                .border_t_1()
                .border_color(cx.theme().border);
            let row = if floating || side == TaskPaneSide::Right {
                row.child(main).child(pane)
            } else {
                row.child(pane).child(main)
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
                        .child(
                            Button::new("toggle")
                                .label(if visible {
                                    "隐藏窗格"
                                } else {
                                    "显示窗格"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| pane.toggle(cx));
                                })),
                        )
                        .child(
                            Button::new("dock-left")
                                .label("停靠左")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        pane.set_side(TaskPaneSide::Left, cx)
                                    });
                                })),
                        )
                        .child(
                            Button::new("dock-right")
                                .label("停靠右")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        pane.set_side(TaskPaneSide::Right, cx)
                                    });
                                })),
                        )
                        .child(
                            Button::new("float")
                                .label(if floating { "回到停靠" } else { "浮动" })
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.toggle_float(window, cx);
                                })),
                        )
                        .child(Button::new("wider").label("加宽").on_click(
                            cx.listener(|this, _event, _window, cx| this.resize_by(40.0, cx)),
                        ))
                        .child(Button::new("narrower").label("收窄").on_click(
                            cx.listener(|this, _event, _window, cx| this.resize_by(-40.0, cx)),
                        ))
                        .child(
                            Button::new("closable")
                                .label(if closable {
                                    "关闭按钮：开"
                                } else {
                                    "关闭按钮：关"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        let next = !pane.is_closable();
                                        pane.set_closable(next, cx);
                                    });
                                })),
                        )
                        .child(
                            Button::new("auto-hide")
                                .label(if auto_hide {
                                    "自动隐藏：开"
                                } else {
                                    "自动隐藏：关"
                                })
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| {
                                        let next = !pane.is_auto_hide();
                                        pane.set_auto_hide(next, cx);
                                    });
                                })),
                        )
                        .child(
                            Button::new("next-tab")
                                .label("下一个页签")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pane.update(cx, |pane, cx| pane.cycle_tab(1, cx));
                                })),
                        )
                        .child(div().text_sm().child(format!(
                            "宽 {width:.0} · {}{}{} · {status}",
                            if floating { "浮动" } else { "停靠" },
                            if visible { "" } else { " · 已隐藏" },
                            if collapsed { " · 已收起" } else { "" }
                        ))),
                )
                .child(row)
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1120.), px(720.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Task Pane Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TaskPaneApp::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| TaskPaneApp::new(window, cx)).into()
    }
}

mod canvas_a11y {
    //! 画布语义（无障碍）示例
    //!
    //! 演示 `canvas_a11y`：自绘画布上的每个对象都会作为**合成子节点**写入无障碍树
    //! （角色 / 名称 / 值 / 矩形 / 选中与焦点状态），于是屏幕阅读器可以枚举、定位、
    //! 点击这些对象 —— 尽管画布上一个字都没画、树里也只有一个画布元素。
    //!
    //! 三种输入路径等价：
    //! - **指针**：点击对象 = 聚焦 + 选中（命中测试与视觉叠放一致，后画者在上）；
    //! - **键盘**：← → ↑ ↓ 沿阅读顺序移动焦点，`Home` / `End` 跳首 / 末，`Enter` / `Space`
    //!   选中焦点对象，`Esc` 清除选中；
    //! - **无障碍动作**：对象声明了 `Click` / `Focus`，辅助技术触发的动作经
    //!   `on_canvas_a11y_action` 回到应用。
    //!
    //! 运行：`cargo run -p showcase --bin canvas_a11y`

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::canvas_a11y::{CanvasNode, CanvasSemantics};
    use rgpui::{
        AccessibleAction, App, BorderStyle, Bounds, Button, Context, Corners, Edges, FocusHandle,
        Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point,
        Render, Role, SharedString, Styled as _, TitlebarOptions, Window, WindowBounds,
        WindowOptions, canvas, div, point, prelude::*, px, quad, size, transparent_black,
    };
    use rgpui_platform::application;

    const CARD_RADIUS: Pixels = px(12.0);
    const HALO: Pixels = px(5.0);

    #[derive(Clone)]
    struct DemoObject {
        /// 稳定标识（语义节点 id）。
        id: u64,
        /// 无障碍名称。
        label: SharedString,
        /// 无障碍值。
        value: SharedString,
        /// 画布局部坐标下的矩形。
        bounds: Bounds<Pixels>,
        /// 填充色。
        tone: Hsla,
        /// 是否画成圆形（正方形 + 半边长圆角）。
        circle: bool,
    }

    impl DemoObject {
        /// 圆角（圆形对象取半边长）。
        fn corners(&self) -> Corners<Pixels> {
            if self.circle {
                Corners::all(self.bounds.size.width / 2.0)
            } else {
                Corners::all(CARD_RADIUS)
            }
        }
    }

    struct CanvasA11yDemo {
        /// 画布对象（按绘制顺序，后画者在上）。
        objects: Vec<DemoObject>,
        /// 无障碍 / 键盘焦点对象。
        focused: Option<u64>,
        /// 选中对象。
        selected: Option<u64>,
        /// 画布原点（窗口坐标，画布预绘制时静默记录）。
        canvas_origin: Point<Pixels>,
        /// 焦点句柄（键盘事件挂在这里）。
        focus_handle: FocusHandle,
        /// 状态提示。
        status: SharedString,
    }

    impl CanvasA11yDemo {
        /// 创建演示视图。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let accent = cx.theme().accent;
            let info = cx.theme().info;
            let success = cx.theme().success;
            let warning = cx.theme().warning;
            let secondary = cx.theme().secondary;
            let primary = cx.theme().primary;

            let objects = vec![
                DemoObject {
                    id: 1,
                    label: "标题文本框".into(),
                    value: "季度总结".into(),
                    bounds: Bounds::new(point(px(24.0), px(20.0)), size(px(240.0), px(60.0))),
                    tone: accent,
                    circle: false,
                },
                DemoObject {
                    id: 2,
                    label: "图表卡片".into(),
                    value: "12 个数据点".into(),
                    bounds: Bounds::new(point(px(288.0), px(20.0)), size(px(280.0), px(160.0))),
                    tone: info,
                    circle: false,
                },
                DemoObject {
                    id: 3,
                    label: "圆形徽标".into(),
                    value: "徽标".into(),
                    bounds: Bounds::new(point(px(24.0), px(104.0)), size(px(120.0), px(120.0))),
                    tone: success,
                    circle: true,
                },
                DemoObject {
                    id: 4,
                    label: "表格卡片".into(),
                    value: "6 行 × 4 列".into(),
                    bounds: Bounds::new(point(px(168.0), px(104.0)), size(px(100.0), px(120.0))),
                    tone: warning,
                    circle: false,
                },
                // 故意压在图表卡片上：命中测试应取绘制顺序靠后的这一个。
                DemoObject {
                    id: 5,
                    label: "重叠卡片".into(),
                    value: "覆盖在图卡右下角".into(),
                    bounds: Bounds::new(point(px(470.0), px(120.0)), size(px(200.0), px(150.0))),
                    tone: secondary,
                    circle: false,
                },
                DemoObject {
                    id: 6,
                    label: "备注卡片".into(),
                    value: "3 条批注".into(),
                    bounds: Bounds::new(point(px(60.0), px(272.0)), size(px(300.0), px(90.0))),
                    tone: primary,
                    circle: false,
                },
            ];

            Self {
                objects,
                focused: Some(1),
                selected: None,
                canvas_origin: point(px(0.0), px(0.0)),
                focus_handle: cx.focus_handle(),
                status: "点对象或用 ← → ↑ ↓ 移动焦点".into(),
            }
        }

        /// 本帧的画布语义（含焦点 / 选中状态）。
        fn semantics(&self) -> CanvasSemantics {
            let mut semantics = CanvasSemantics::new();
            for object in &self.objects {
                let mut node = CanvasNode::new(
                    object.id,
                    Role::GraphicsObject,
                    object.label.clone(),
                    object.bounds,
                )
                .with_value(object.value.clone())
                .with_actions([AccessibleAction::Click, AccessibleAction::Focus]);
                node.set_selected(self.selected == Some(object.id));
                node.set_focused(self.focused == Some(object.id));
                semantics.push(node);
            }
            semantics
        }

        /// 对象名称（按 id）。
        fn label_of(&self, id: u64) -> String {
            self.objects
                .iter()
                .find(|object| object.id == id)
                .map(|object| object.label.to_string())
                .unwrap_or_else(|| format!("#{id}"))
        }

        /// 阅读顺序的可读文本（先按行，行内自左向右）。
        fn reading_order_text(&self) -> String {
            self.semantics()
                .reading_order()
                .iter()
                .map(|id| self.label_of(*id))
                .collect::<Vec<_>>()
                .join(" → ")
        }

        /// 聚焦某个对象。
        fn focus_object(&mut self, id: u64, cx: &mut Context<Self>) {
            if self.focused == Some(id) {
                return;
            }
            self.focused = Some(id);
            self.status = format!("焦点：{}（{}）", self.label_of(id), self.value_of(id)).into();
            cx.notify();
        }

        /// 对象的值（按 id）。
        fn value_of(&self, id: u64) -> String {
            self.objects
                .iter()
                .find(|object| object.id == id)
                .map(|object| object.value.to_string())
                .unwrap_or_default()
        }

        /// 沿阅读顺序移动焦点。
        fn move_focus(&mut self, forward: bool, cx: &mut Context<Self>) {
            if let Some(next) = self
                .semantics()
                .next_in_reading_order(self.focused, forward)
            {
                self.focus_object(next, cx);
            }
        }

        /// 焦点移到阅读顺序首 / 末个对象。
        fn focus_boundary(&mut self, last: bool, cx: &mut Context<Self>) {
            let semantics = self.semantics();
            let target = if last {
                semantics.next_in_reading_order(None, false)
            } else {
                semantics.first_in_reading_order()
            };
            if let Some(id) = target {
                self.focus_object(id, cx);
            }
        }

        /// 选中焦点对象。
        fn select_focused(&mut self, cx: &mut Context<Self>) {
            let Some(id) = self.focused else {
                return;
            };
            self.selected = Some(id);
            self.status = format!("选中：{}（{}）", self.label_of(id), self.value_of(id)).into();
            cx.notify();
        }

        /// 清除选中。
        fn clear_selection(&mut self, cx: &mut Context<Self>) {
            self.selected = None;
            self.status = "已清除选中".into();
            cx.notify();
        }

        /// 指针按下：按窗口坐标命中对象（画布局部坐标 = 窗口坐标 - 画布原点）。
        fn on_pointer_down(&mut self, window_point: Point<Pixels>, cx: &mut Context<Self>) {
            let local = window_point - self.canvas_origin;
            match self.semantics().hit_test(local) {
                Some(id) => {
                    self.focused = Some(id);
                    self.selected = Some(id);
                    self.status =
                        format!("点击：{}（{}）", self.label_of(id), self.value_of(id)).into();
                }
                None => {
                    self.selected = None;
                    self.status = "点击空白：清除选中".into();
                }
            }
            cx.notify();
        }

        /// 键盘：方向键沿阅读顺序移动焦点。
        fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
            match event.keystroke.key.as_str() {
                "right" | "down" => self.move_focus(true, cx),
                "left" | "up" => self.move_focus(false, cx),
                "home" => self.focus_boundary(false, cx),
                "end" => self.focus_boundary(true, cx),
                "enter" | "space" => self.select_focused(cx),
                "escape" => self.clear_selection(cx),
                _ => {}
            }
        }

        /// 无障碍动作：与指针 / 键盘等价。
        fn on_canvas_action(&mut self, id: u64, action: AccessibleAction, cx: &mut Context<Self>) {
            match action {
                AccessibleAction::Click => {
                    self.focused = Some(id);
                    self.selected = Some(id);
                    self.status =
                        format!("无障碍点击：{}（{}）", self.label_of(id), self.value_of(id))
                            .into();
                }
                AccessibleAction::Focus => {
                    self.focused = Some(id);
                    self.status = format!("无障碍聚焦：{}", self.label_of(id)).into();
                }
                _ => return,
            }
            cx.notify();
        }
    }

    fn translate(bounds: Bounds<Pixels>, origin: Point<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            point(bounds.origin.x + origin.x, bounds.origin.y + origin.y),
            bounds.size,
        )
    }

    fn inflate(bounds: Bounds<Pixels>, by: Pixels) -> Bounds<Pixels> {
        Bounds::new(
            point(bounds.origin.x - by, bounds.origin.y - by),
            size(bounds.size.width + by * 2.0, bounds.size.height + by * 2.0),
        )
    }

    fn paint_objects(
        objects: &[DemoObject],
        origin: Point<Pixels>,
        focused: Option<u64>,
        selected: Option<u64>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let ring = cx.theme().ring;
        let selection = cx.theme().selection;
        let border = cx.theme().border;

        for object in objects {
            let is_focused = focused == Some(object.id);
            let is_selected = selected == Some(object.id);
            if !is_focused && !is_selected {
                continue;
            }
            let halo = inflate(translate(object.bounds, origin), HALO);
            window.paint_quad(quad(
                halo,
                object.corners(),
                if is_focused { ring } else { selection },
                Edges::all(px(0.0)),
                transparent_black(),
                BorderStyle::Solid,
            ));
        }

        for object in objects {
            window.paint_quad(quad(
                translate(object.bounds, origin),
                object.corners(),
                object.tone,
                Edges::all(px(1.0)),
                border,
                BorderStyle::Solid,
            ));
        }
    }

    impl Render for CanvasA11yDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = cx.theme().clone();
            let semantics = self.semantics();
            let objects = self.objects.clone();
            let (focused, selected) = (self.focused, self.selected);

            // 画布：铺满容器；预绘制时记下原点（静默，不触发重绘）。
            let origin_recorder = cx.entity().downgrade();
            let surface = canvas(
                move |bounds, _window, cx| {
                    origin_recorder
                        .update(cx, |this, _| this.canvas_origin = bounds.origin)
                        .ok();
                },
                move |bounds, (), window, cx| {
                    paint_objects(&objects, bounds.origin, focused, selected, window, cx)
                },
            )
            .size_full()
            .id("canvas-a11y-surface")
            .a11y(Role::Canvas, semantics)
            .a11y_label("画布对象")
            .on_canvas_a11y_action({
                let weak = cx.entity().downgrade();
                let focus_handle = self.focus_handle.clone();
                move |id, action, _data, window, cx| {
                    // 辅助技术操作对象时，也把键盘焦点交给画布容器。
                    window.focus(&focus_handle, cx);
                    weak.update(cx, |this, cx| this.on_canvas_action(id, action, cx))
                        .ok();
                }
            });

            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(theme.background)
                .text_color(theme.foreground)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(div().text_sm().child("画布语义（无障碍）"))
                        .child(
                            Button::new("canvas-a11y-prev")
                                .label("上一个")
                                .on_click(cx.listener(|this, _, _, cx| this.move_focus(false, cx))),
                        )
                        .child(
                            Button::new("canvas-a11y-next")
                                .label("下一个")
                                .on_click(cx.listener(|this, _, _, cx| this.move_focus(true, cx))),
                        )
                        .child(
                            Button::new("canvas-a11y-select")
                                .label("选中焦点")
                                .on_click(cx.listener(|this, _, _, cx| this.select_focused(cx))),
                        )
                        .child(
                            Button::new("canvas-a11y-clear")
                                .label("清除选中")
                                .on_click(cx.listener(|this, _, _, cx| this.clear_selection(cx))),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "对象 {} · 焦点 {} · 选中 {}",
                                    self.objects.len(),
                                    self.focused
                                        .map(|id| self.label_of(id))
                                        .unwrap_or_else(|| "无".into()),
                                    self.selected
                                        .map(|id| self.label_of(id))
                                        .unwrap_or_else(|| "无".into()),
                                )),
                        ),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("阅读顺序：{}", self.reading_order_text())),
                )
                .child(
                    div()
                        .id("canvas-a11y-host")
                        .track_focus(&self.focus_handle)
                        .key_context("CanvasA11y")
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .relative()
                        .overflow_hidden()
                        .m_3()
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(theme.border)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                this.on_pointer_down(event.position, cx);
                            }),
                        )
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                            this.on_key_down(event, cx);
                        }))
                        .child(surface),
                )
                .child(
                    div()
                        .px_3()
                        .pb_3()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(self.status.clone()),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("画布无障碍示例".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| CanvasA11yDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| CanvasA11yDemo::new(window, cx)).into()
    }
}

mod page_view {
    //! 文档页视图演示：`PageView` 分页画布 + `Viewport` 缩放/平移。
    //!
    //! 三页 A4 文档排布（单页 / 对开 / 封面对开可切），`PageView` 绘制纸页与页眉页脚，
    //! 正文按 `PageLayout::content_rect` 导出的矩形叠加（字号随缩放同步）。滚轮 `Ctrl/⌘`
    //! 以光标为中心缩放、否则平移；左键拖拽平移；工具栏可缩小/放大/100%/适应窗口/切换排布。

    #![cfg_attr(target_family = "wasm", no_main)]

    use rgpui::page_view::{PageArrangement, PageLabels, PageLayout, PageSetup, PageView};
    use rgpui::viewport::Viewport;
    use rgpui::{
        App, Bounds, Button, Context, FocusHandle, IntoElement, MouseButton, MouseDownEvent,
        MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
        SharedString, Size, Styled, Window, WindowBounds, WindowOptions, div, h_flex, prelude::*,
        px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    const PAGE_COUNT: usize = 3;

    const ARRANGEMENTS: [PageArrangement; 3] = [
        PageArrangement::Single,
        PageArrangement::TwoUp,
        PageArrangement::TwoUpCover,
    ];

    fn arrangement_label(arrangement: PageArrangement) -> &'static str {
        match arrangement {
            PageArrangement::Single => "单页",
            PageArrangement::TwoUp => "对开",
            PageArrangement::TwoUpCover => "封面对开",
        }
    }

    struct DocLabels {
        /// 页眉标题。
        title: SharedString,
    }

    impl PageLabels for DocLabels {
        fn header(&self, _page: usize, _cx: &App) -> SharedString {
            self.title.clone()
        }

        fn footer(&self, page: usize, _cx: &App) -> SharedString {
            format!("第 {} / {PAGE_COUNT} 页", page + 1).into()
        }
    }

    fn page_text(page: usize) -> &'static str {
        match page {
            0 => {
                "文档页视图（Document Page View）\n\n\
              本演示用 PageView 渲染分页纸页：纸张尺寸、页边距、页眉页脚高度与页间距\
              共同决定文档坐标，屏幕映射交给 Viewport。\n\n\
              正文由应用按 PageLayout::content_rect(page) 的矩形叠加，\
              框架只出几何与纸页外观，分页/断行算法留在应用层。"
            }
            1 => {
                "坐标系统\n\n\
              screen = document × zoom + offset。\n\n\
              滚轮 Ctrl/⌘ 以光标为锚点缩放；普通滚轮平移；左键拖拽亦可平移。\n\n\
              页与页之间保留 gap 间距，命中判定 page_at 会把间距内的点归给上一页；\
              对开排布（TwoUp / 封面对开 TwoUpCover）下每行两页，但分页/断行仍由应用决定。"
            }
            _ => {
                "内容区\n\n\
              content_rect 扣除了左右边距与页眉页脚占位，\
              是正文可安全排布的竖直带状区域。\n\n\
              页眉贴内容区上方、页脚贴内容区下方并居中，随缩放一起移动。\n\n\
              —— 演示结束 ——"
            }
        }
    }

    struct PageViewDemo {
        /// 分页布局。
        layout: PageLayout,
        /// 当前页面排布方式。
        arrangement: PageArrangement,
        /// 视口（缩放 + 平移）。
        viewport: Viewport,
        /// 焦点句柄。
        focus_handle: FocusHandle,
        /// 画布可用尺寸。
        view_size: Size<Pixels>,
        /// 缩放/平移锚点（最近鼠标位置）。
        anchor: Point<Pixels>,
        /// 是否正在拖拽平移。
        dragging: bool,
        /// 上次指针位置。
        last_pointer: Point<Pixels>,
    }

    impl PageViewDemo {
        /// 创建演示视图。
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let setup = PageSetup::default();
            let arrangement = PageArrangement::Single;
            let layout = PageLayout::new(setup, PAGE_COUNT).with_arrangement(arrangement);
            Self {
                layout,
                arrangement,
                viewport: Viewport::new()
                    .with_zoom(0.55)
                    .with_offset(px_point(120.0, 16.0)),
                focus_handle: cx.focus_handle(),
                view_size: size(px(900.0), px(640.0)),
                anchor: px_point(450.0, 320.0),
                dragging: false,
                last_pointer: px_point(0.0, 0.0),
            }
        }
    }

    fn px_point(x: f32, y: f32) -> Point<Pixels> {
        rgpui::point(px(x), px(y))
    }

    impl Render for PageViewDemo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if window.bounds().size != self.view_size && !self.dragging {
                self.view_size = window.bounds().size;
            }

            let viewport = self.viewport;
            let layout = self.layout.clone();
            let zoom = viewport.zoom();
            let font_size = px((14.0 * zoom).max(4.0));

            // 正文叠加：按文档矩形换算到屏幕，字号随缩放同步。
            let mut overlays: Vec<rgpui::AnyElement> = Vec::new();
            for page in 0..layout.page_count() {
                let content = layout.content_rect(page);
                let origin = viewport.document_to_screen(content.origin);
                let size = viewport.document_size_to_screen(content.size);
                overlays.push(
                    div()
                        .absolute()
                        .left(origin.x)
                        .top(origin.y)
                        .w(size.width)
                        .h(size.height)
                        .p(px(4.0))
                        .text_size(font_size)
                        .line_height(rgpui::relative(1.4))
                        .text_color(rgb(0x1e293b))
                        .child(page_text(page))
                        .into_any_element(),
                );
            }

            v_flex()
                .id("page-view-demo")
                .track_focus(&self.focus_handle)
                .size_full()
                .bg(rgb(0xf1f5f9))
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xcbd5e1))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(rgpui::FontWeight::SEMIBOLD)
                                .child("文档页视图（PageView）"),
                        )
                        .child(
                            Button::new("pv-zoom-out")
                                .label("缩小")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_out_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("pv-zoom-in")
                                .label("放大")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let anchor = this.anchor;
                                    this.viewport.zoom_in_at(anchor);
                                    cx.notify();
                                })),
                        )
                        .child(Button::new("pv-100").label("100%").on_click(cx.listener(
                            |this, _, _, cx| {
                                let anchor = this.anchor;
                                this.viewport.set_zoom_at(1.0, anchor);
                                cx.notify();
                            },
                        )))
                        .child(
                            Button::new("pv-fit")
                                .label("适应窗口")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let view = this.view_size;
                                    let doc = this.layout.document_size();
                                    this.viewport.fit(doc, view, px(24.0));
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("pv-arrangement")
                                .label(format!("排布：{}", arrangement_label(self.arrangement)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    // 依固定顺序循环，并同步重建布局（排布影响文档尺寸与命中）。
                                    let index = match this.arrangement {
                                        PageArrangement::Single => 0,
                                        PageArrangement::TwoUp => 1,
                                        PageArrangement::TwoUpCover => 2,
                                    };
                                    let next = ARRANGEMENTS[(index + 1) % ARRANGEMENTS.len()];
                                    this.arrangement = next;
                                    this.layout = this.layout.clone().with_arrangement(next);
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(0x64748b))
                                .child(format!(
                                    "缩放 {:.0}% · {} 页 · {} 行",
                                    zoom * 100.0,
                                    self.layout.page_count(),
                                    self.layout.row_count()
                                )),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .flex_grow_1()
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                this.dragging = true;
                                this.last_pointer = event.position;
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                this.dragging = false;
                                cx.notify();
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            this.anchor = event.position;
                            if this.dragging {
                                let delta = event.position - this.last_pointer;
                                this.last_pointer = event.position;
                                this.viewport.pan_by(delta);
                            }
                            cx.notify();
                        }))
                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                            this.viewport.apply_wheel(event, px(50.0));
                            cx.notify();
                        }))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(PageView::new(
                                    layout,
                                    viewport,
                                    DocLabels {
                                        title: "rgpui 文档套件基座".into(),
                                    },
                                )),
                        )
                        .children(overlays),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::init_all(cx);
            let bounds = Bounds::centered(None, size(px(980.0), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| PageViewDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| PageViewDemo::new(window, cx)).into()
    }
}

mod drag_drop {

    //! 拖拽示例：展示元素拖动、拖放落点与拖动预览。
    //! 用法：左侧 tab 选「drag_drop」，按住色块拖动到别处，观察落点反馈。
    use rgpui::{
        App, Bounds, Context, Half, Hsla, Pixels, Point, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    #[derive(Clone, Copy)]
    struct DragInfo {
        ix: usize,
        color: Hsla,
        position: Point<Pixels>,
    }

    impl DragInfo {
        fn new(ix: usize, color: Hsla) -> Self {
            Self {
                ix,
                color,
                position: Point::default(),
            }
        }

        fn position(mut self, pos: Point<Pixels>) -> Self {
            self.position = pos;
            self
        }
    }

    impl Render for DragInfo {
        fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
            let size = rgpui::size(px(120.), px(50.));

            div()
                .pl(self.position.x - size.width.half())
                .pt(self.position.y - size.height.half())
                .child(
                    div()
                        .flex()
                        .justify_center()
                        .items_center()
                        .w(size.width)
                        .h(size.height)
                        .bg(self.color.opacity(0.5))
                        .text_color(rgpui::white())
                        .text_xs()
                        .shadow_md()
                        .child(format!("Item {}", self.ix)),
                )
        }
    }

    struct DragDrop {
        drop_on: Option<DragInfo>,
    }

    impl DragDrop {
        fn new() -> Self {
            Self { drop_on: None }
        }
    }

    impl Render for DragDrop {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let items = [rgpui::blue(), rgpui::red(), rgpui::green()];

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_5()
                .bg(rgpui::white())
                .justify_center()
                .items_center()
                .text_color(rgb(0x333333))
                .child(div().text_xl().text_center().child("Drop & Drop"))
                .child(
                    div()
                        .w_full()
                        .mb_10()
                        .justify_center()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .items_center()
                        .children(items.into_iter().enumerate().map(|(ix, color)| {
                            let drag_info = DragInfo::new(ix, color);

                            div()
                                .id(("item", ix))
                                .size_32()
                                .flex()
                                .justify_center()
                                .items_center()
                                .border_2()
                                .border_color(color)
                                .text_color(color)
                                .cursor_move()
                                .hover(|this| this.bg(color.opacity(0.2)))
                                .child(format!("Item ({})", ix))
                                .on_drag(drag_info, |info: &DragInfo, position, _, cx| {
                                    cx.new(|_| info.position(position))
                                })
                        })),
                )
                .child(
                    div()
                        .id("drop-target")
                        .w_128()
                        .h_32()
                        .flex()
                        .justify_center()
                        .items_center()
                        .border_3()
                        .border_color(
                            self.drop_on
                                .map(|info| info.color)
                                .unwrap_or(rgpui::black()),
                        )
                        .when_some(self.drop_on, |this, info| this.bg(info.color.opacity(0.5)))
                        .on_drop(cx.listener(|this, info: &DragInfo, _, _| {
                            this.drop_on = Some(*info);
                        }))
                        .child("拖到此处"),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| DragDrop::new()),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| DragDrop::new()).into()
    }
}

mod focus_visible {

    //! 焦点可见性示例：展示键盘聚焦时的高亮边框（focus ring）。
    //! 用法：左侧 tab 选「focus_visible」，用 Tab 在按钮间移动，观察焦点样式。
    use rgpui::{
        App, Bounds, Context, Div, ElementId, FocusHandle, KeyBinding, SharedString, Stateful,
        Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    actions!(focus_visible_example, [Tab, TabPrev, Quit]);

    struct Example {
        focus_handle: FocusHandle,
        items: Vec<(FocusHandle, &'static str)>,
        message: SharedString,
    }

    impl Example {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let items = vec![
                (
                    cx.focus_handle().tab_index(1).tab_stop(true),
                    "Button with .focus() - always shows border when focused",
                ),
                (
                    cx.focus_handle().tab_index(2).tab_stop(true),
                    "Button with .focus_visible() - only shows border with keyboard",
                ),
                (
                    cx.focus_handle().tab_index(3).tab_stop(true),
                    "Button with both .focus() and .focus_visible()",
                ),
            ];

            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);

            Self {
                focus_handle,
                items,
                message: SharedString::from(
                    "Try clicking vs tabbing! Click shows no border, Tab shows border.",
                ),
            }
        }

        fn on_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_next(cx);
            self.message = SharedString::from("Pressed Tab - focus-visible border should appear!");
        }

        fn on_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_prev(cx);
            self.message =
                SharedString::from("Pressed Shift-Tab - focus-visible border should appear!");
        }

        fn on_quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
            cx.quit();
        }
    }

    impl Render for Example {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            fn button_base(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
                div()
                    .id(id)
                    .h_16()
                    .w_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .bg(rgpui::rgb(0x2563eb))
                    .text_color(rgpui::white())
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgpui::rgb(0x1d4ed8)))
                    .child(label)
            }

            div()
            .id("app")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_tab))
            .on_action(cx.listener(Self::on_tab_prev))
            .on_action(cx.listener(Self::on_quit))
            .size_full()
            .flex()
            .flex_col()
            .p_8()
            .gap_6()
            .bg(rgpui::rgb(0xf3f4f6))
            .child(
                div()
                    .text_2xl()
                    .font_weight(rgpui::FontWeight::BOLD)
                    .text_color(rgpui::rgb(0x111827))
                    .child("CSS focus-visible Demo"),
            )
            .child(
                div()
                    .p_4()
                    .rounded_md()
                    .bg(rgpui::rgb(0xdbeafe))
                    .text_color(rgpui::rgb(0x1e3a8a))
                    .child(self.message.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(rgpui::FontWeight::BOLD)
                                    .text_color(rgpui::rgb(0x374151))
                                    .child("1. Regular .focus() - always visible:"),
                            )
                            .child(
                                button_base("button1", self.items[0].1)
                                    .track_focus(&self.items[0].0)
                                    .focus(|style| {
                                        style.border_4().border_color(rgpui::rgb(0xfbbf24))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message =
                                            "Clicked button 1 - focus border is visible!".into();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(rgpui::FontWeight::BOLD)
                                    .text_color(rgpui::rgb(0x374151))
                                    .child("2. New .focus_visible() - only keyboard:"),
                            )
                            .child(
                                button_base("button2", self.items[1].1)
                                    .track_focus(&self.items[1].0)
                                    .focus_visible(|style| {
                                        style.border_4().border_color(rgpui::rgb(0x10b981))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message =
                                            "Clicked button 2 - no border! Try Tab instead.".into();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(rgpui::FontWeight::BOLD)
                                    .text_color(rgpui::rgb(0x374151))
                                    .child(
                                        "3. Both .focus() (yellow) and .focus_visible() (green):",
                                    ),
                            )
                            .child(
                                button_base("button3", self.items[2].1)
                                    .track_focus(&self.items[2].0)
                                    .focus(|style| {
                                        style.border_4().border_color(rgpui::rgb(0xfbbf24))
                                    })
                                    .focus_visible(|style| {
                                        style.border_4().border_color(rgpui::rgb(0x10b981))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.message =
                                            "Clicked button 3 - yellow border. Tab shows green!"
                                                .into();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.bind_keys([
                KeyBinding::new("tab", Tab, None),
                KeyBinding::new("shift-tab", TabPrev, None),
                KeyBinding::new("cmd-q", Quit, None),
            ]);

            let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Example::new(window, cx)),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| Example::new(window, cx)).into()
    }
}

mod mouse_pressure {

    //! 鼠标压感示例：展示压力敏感输入的状态读取与可视化。
    //! 用法：左侧 tab 选「mouse_pressure」，在触控板/压感笔上按压观察数值变化。
    use rgpui::{
        App, Bounds, Context, MousePressureEvent, PressureStage, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct MousePressureExample {
        pressure_stage: PressureStage,
        pressure_amount: f32,
    }

    impl Render for MousePressureExample {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .bg(rgb(0x505050))
                .size(px(500.0))
                .justify_center()
                .items_center()
                .shadow_lg()
                .border_1()
                .border_color(rgb(0x0000ff))
                .text_xl()
                .text_color(rgb(0xffffff))
                .child(format!("Pressure stage: {:?}", self.pressure_stage))
                .child(format!("Pressure amount: {:.2}", self.pressure_amount))
                .on_mouse_pressure(cx.listener(Self::on_mouse_pressure))
        }
    }

    impl MousePressureExample {
        fn on_mouse_pressure(
            &mut self,
            pressure_event: &MousePressureEvent,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.pressure_amount = pressure_event.pressure;
            self.pressure_stage = pressure_event.stage;

            cx.notify();
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| MousePressureExample {
                        pressure_stage: PressureStage::Zero,
                        pressure_amount: 0.0,
                    })
                },
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| MousePressureExample {
            pressure_stage: PressureStage::Zero,
            pressure_amount: 0.0,
        })
        .into()
    }
}

mod native_menu {
    //! OS 原生弹出菜单示例
    //!
    //! 演示 [`NativeMenu`](rgpui::NativeMenu) + [`Window::show_native_menu`]：菜单交给系统
    //! 自己的控件画出来（Windows `TrackPopupMenuEx`、macOS `NSMenu`），选中结果按菜单项的
    //! `id` 经通道异步回传。平台没有这项能力时（Linux 的窗口菜单只能由窗管自己画）返回
    //! `None`，示例如实提示，并给出框架自绘菜单（[`ContextMenuExt`]）作为替代路径。
    //!
    //! 运行：`cargo run -p showcase --bin native_menu`

    use rgpui::{
        App, Bounds, Button, Context, ContextMenuExt as _, Entity, IntoElement, NativeMenu,
        ParentElement, PopupMenuItem, Render, SharedString, Styled as _, TitlebarOptions, Window,
        WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    struct NativeMenuApp {
        /// 指向自身的句柄，供菜单项回调更新状态
        self_handle: Entity<Self>,
        /// 最近一次菜单交互的结果文本
        status: SharedString,
        /// 「自动换行」开关的当前状态，原生菜单里以勾选态呈现
        word_wrap: bool,
    }

    impl NativeMenuApp {
        /// 创建示例应用
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                self_handle: cx.entity(),
                status: "点击按钮或右键下方区域试试".into(),
                word_wrap: true,
            }
        }

        /// 按当前状态构建要弹出的原生菜单。
        ///
        /// 勾选态与禁用态都在描述阶段给出，平台层只负责把它们翻译成原生菜单的属性，
        /// 所以这份构建在两个平台上表现一致。
        fn build_menu(&self) -> NativeMenu {
            NativeMenu::new()
                .action("复制", "copy")
                .action("粘贴", "paste")
                .disabled("删除", "delete", true)
                .separator()
                .checked("自动换行", "word-wrap", self.word_wrap)
                .submenu("打开方式", |menu| {
                    menu.action("新窗口", "open-new")
                        .action("默认程序", "open-default")
                })
                .separator()
                .action("关于 rgpui", "about")
        }

        /// 处理一次菜单选择：切换开关项，其余只更新状态文本。
        fn apply_selection(&mut self, id: &str) {
            match id {
                "word-wrap" => {
                    self.word_wrap = !self.word_wrap;
                    self.status =
                        format!("自动换行已{}", if self.word_wrap { "开启" } else { "关闭" })
                            .into();
                }
                other => self.status = format!("菜单选中：{other}").into(),
            }
        }

        /// 在鼠标位置弹出 OS 原生菜单，并把选中结果回写到状态文本。
        fn show_native(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            let menu = self.build_menu();
            let Some(receiver) = window.show_native_menu(window.mouse_position(), menu) else {
                self.status =
                    "本平台没有原生弹出菜单，请改用框架自绘的 PopupMenu（右键下方区域）".into();
                cx.notify();
                return;
            };

            // 菜单在平台自己的事件循环里模态运行，所以结果只能等：用户关掉菜单时通道被丢弃。
            cx.spawn(async move |this, cx| {
                let text = match receiver.await {
                    Ok(id) => Selected::Picked(id),
                    Err(_) => Selected::Dismissed,
                };
                cx.update(|cx| {
                    let _ = this.update(cx, |this, cx| {
                        match text {
                            Selected::Picked(id) => this.apply_selection(&id),
                            Selected::Dismissed => {
                                this.status = "菜单已关闭（未选择任何项）".into()
                            }
                        }
                        cx.notify();
                    });
                });
            })
            .detach();
        }
    }

    enum Selected {
        Picked(SharedString),
        Dismissed,
    }

    impl Render for NativeMenuApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let native_handle = self.self_handle.clone();
            let menu_handle = self.self_handle.clone();
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .bg(rgb(0x2b2b2b))
                .size_full()
                .text_color(rgb(0xffffff))
                .child(div().text_xl().child("OS 原生弹出菜单示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(
                    Button::new("native")
                        .label("在鼠标位置弹出原生菜单")
                        .on_click(move |_, window, cx| {
                            native_handle.update(cx, |this, cx| this.show_native(window, cx));
                        }),
                )
                .child(
                    // 框架自绘菜单：跨平台都可用，是原生菜单缺位时的替代
                    div()
                        .id("fallback-area")
                        .flex()
                        .items_center()
                        .justify_center()
                        .w_full()
                        .h(px(120.))
                        .rounded(px(8.))
                        .bg(rgb(0x3c3f41))
                        .child("右键此处：框架自绘的 ContextMenu")
                        .context_menu(move |menu, _, _| {
                            let copy_handle = menu_handle.clone();
                            let about_handle = menu_handle.clone();
                            menu.item(PopupMenuItem::new("复制").on_click(move |_, _, cx| {
                                copy_handle.update(cx, |this, cx| {
                                    this.status = "框架菜单选中：复制".into();
                                    cx.notify();
                                });
                            }))
                            .item(PopupMenuItem::new("粘贴").disabled(true))
                            .separator()
                            .item(
                                PopupMenuItem::new("关于 rgpui").on_click(move |_, _, cx| {
                                    about_handle.update(cx, |this, cx| {
                                        this.status = "框架菜单选中：关于 rgpui".into();
                                        cx.notify();
                                    });
                                }),
                            )
                        }),
                )
        }
    }

    pub fn run_example() {
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(560.), px(360.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Native Menu Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(NativeMenuApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(NativeMenuApp::new).into()
    }
}

mod native_notification {
    //! 原生系统通知示例
    //!
    //! 演示 `App::show_notification` / `App::push_notification`：
    //! 调用经 `Platform` trait 分发到各平台原生实现
    //! （Windows Toast、macOS User Notifications、Linux XDG Desktop Portal）。
    //! 一个按钮直接发送；另一个把发送结果回显到窗口状态文本。
    //!
    //! 运行：`cargo run -p showcase --bin interaction_gallery -- native_notification`

    use rgpui::{
        App, Bounds, Button, Context, Entity, IntoElement, ParentElement, Render, SharedString,
        Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, px,
        rgb, size,
    };
    use rgpui_platform::application;

    /// 通知示例应用：记录最近一次发送结果并渲染状态文本与按钮
    struct NotificationApp {
        /// 指向自身的句柄，供按钮点击回调更新状态使用
        self_handle: Entity<Self>,
        /// 最近一次通知发送结果的展示文本
        status: SharedString,
    }

    impl NotificationApp {
        /// 创建示例应用，初始状态提示用户操作
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                self_handle: cx.entity(),
                status: "点击下方按钮发送原生通知".into(),
            }
        }

        /// 发送一条操作系统原生通知，并按发送结果更新窗口内状态文本。
        ///
        /// `show_notification` 返回 `Result`：平台不支持或通知门户拒绝时
        /// 会得到 `Err`，这里如实展示，便于在 headless 等环境下定位问题。
        fn send_notification(&mut self, title: &str, body: &str, cx: &mut Context<Self>) {
            self.status = match cx.show_notification(title, body) {
                Ok(()) => format!("已发送通知：{title}").into(),
                Err(err) => format!("发送失败：{err}").into(),
            };
            cx.notify();
        }
    }

    impl Render for NotificationApp {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let handle = self.self_handle.clone();
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .bg(rgb(0x2b2b2b))
                .size_full()
                .justify_center()
                .items_center()
                .text_color(rgb(0xffffff))
                .child(div().text_xl().child("原生系统通知示例"))
                .child(div().text_sm().child(self.status.clone()))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            // 直接调用 App 便捷方法，不关心发送结果
                            Button::new("send-simple").label("发送通知").on_click(
                                |_event, _window, cx| {
                                    let _ = cx.push_notification(
                                        "rgpui",
                                        "这是一条来自 rgpui 的原生通知",
                                    );
                                },
                            ),
                        )
                        .child(
                            // 经实体句柄更新状态文本，把成败回显到窗口内
                            Button::new("send-with-status")
                                .label("发送通知并回显结果")
                                .on_click(move |_event, _window, cx| {
                                    handle.update(cx, |this, cx| {
                                        this.send_notification(
                                            "rgpui 带参数的通知",
                                            "标题与内容均由调用方传入",
                                            cx,
                                        );
                                    });
                                }),
                        ),
                )
        }
    }

    pub fn run_example() {
        // 诊断时用 RUST_LOG=info 运行，可看到通知门户调用等内部日志
        rgpui::init_logging();
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(480.), px(260.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Native Notification Example".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(NotificationApp::new),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(NotificationApp::new).into()
    }
}

mod set_menus {

    //! 原生菜单数据示例：展示如何为窗口设置应用菜单与菜单项回调。
    //! 用法：左侧 tab 选「set_menus」，点击窗口顶部菜单项观察控制台输出。
    use rgpui::{
        App, Context, Global, Menu, MenuItem, SharedString, SystemMenuType, Window, WindowOptions,
        actions, div, prelude::*,
    };
    use rgpui_platform::application;

    struct SetMenus;

    impl Render for SetMenus {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // 将已存储的菜单读回并展示：原生菜单栏仅在 macOS 显示为系统全局菜单；
            // 在 Windows/Linux 上 `set_menus` 只存储数据（可通过 `get_menus` 读回），
            // 不会渲染原生菜单栏，因此这里把存储内容直接画出来作为反馈。
            let stored = cx.get_menus().unwrap_or_default();

            div()
            .flex()
            .flex_col()
            .bg(rgpui::white())
            .size_full()
            .p_8()
            .gap_4()
            .text_color(rgpui::black())
            .child(div().text_xl().child("Set Menus Example"))
            .child(
                div()
                    .text_sm()
                    .child("原生应用菜单栏仅在 macOS 显示为系统全局菜单；在 Windows/Linux 上 set_menus 仅存储数据（可用 get_menus 读回），不会渲染原生菜单栏。",
                    ),
            )
            .child(
                div()
                    .id("toggle-view-mode")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgpui::black())
                    .text_color(rgpui::white())
                    .text_sm()
                    .child("切换视图模式（List / Grid，将刷新下方读回结果）")
                    .on_click(cx.listener(|_, _, _, cx| {
                        toggle_check(&ToggleCheck, cx);
                        cx.refresh_windows();
                    })),
            )
            .child(div().text_base().child(format!(
                "已存储的顶层菜单（get_menus 读回，共 {} 个）：",
                stored.len()
            )))
            .children(stored.iter().map(|menu| {
                div().text_sm().child(format!(
                    "· {}{}（{} 项）",
                    menu.name,
                    if menu.disabled { " [禁用]" } else { "" },
                    menu.items.len()
                ))
            }))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.set_global(AppState::new());

            // Bring the menu bar to the foreground (so you can see the menu bar)
            cx.activate(true);
            // Register the `quit` function so it can be referenced
            // by the `MenuItem::action` in the menu bar
            cx.on_action(quit);
            cx.on_action(toggle_check);
            // 原生菜单项动作的落点：macOS 全局菜单被选中、Windows 托盘/跳转列表被点击、
            // Linux 用旧 set_tray API 随菜单项带过去的 Action 被点击
            cx.on_app_menu_action(|action, cx| cx.dispatch_action(action));
            // Add menu items
            set_app_menus(cx);
            cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| SetMenus {}))
                .unwrap();
        });
    }

    #[derive(PartialEq)]
    enum ViewMode {
        List,
        Grid,
    }

    impl ViewMode {
        fn toggle(&mut self) {
            *self = match self {
                ViewMode::List => ViewMode::Grid,
                ViewMode::Grid => ViewMode::List,
            }
        }
    }

    impl From<ViewMode> for SharedString {
        fn from(val: ViewMode) -> Self {
            match val {
                ViewMode::List => "List",
                ViewMode::Grid => "Grid",
            }
            .into()
        }
    }

    struct AppState {
        view_mode: ViewMode,
    }

    impl AppState {
        fn new() -> Self {
            Self {
                view_mode: ViewMode::List,
            }
        }
    }

    impl Global for AppState {}

    fn set_app_menus(cx: &mut App) {
        let app_state = cx.global::<AppState>();
        cx.set_menus([Menu::new("set_menus").items([
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("禁用项", rgpui::NoAction).disabled(true),
            MenuItem::submenu(Menu::new("禁用子菜单").disabled(true)),
            MenuItem::separator(),
            MenuItem::action("List Mode", ToggleCheck)
                .checked(app_state.view_mode == ViewMode::List),
            MenuItem::submenu(
                Menu::new("Mode").items([
                    MenuItem::action(ViewMode::List, ToggleCheck)
                        .checked(app_state.view_mode == ViewMode::List),
                    MenuItem::action(ViewMode::Grid, ToggleCheck)
                        .checked(app_state.view_mode == ViewMode::Grid),
                ]),
            ),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ])]);
    }

    // Associate actions using the `actions!` macro (or `Action` derive macro)
    actions!(set_menus_set_menus, [Quit, ToggleCheck]);

    // Define the quit function that is registered with the App
    fn quit(_: &Quit, cx: &mut App) {
        println!("Gracefully quitting the application...");
        cx.quit();
    }

    fn toggle_check(_: &ToggleCheck, cx: &mut App) {
        let app_state = cx.global_mut::<AppState>();
        app_state.view_mode.toggle();
        set_app_menus(cx);
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        // 该示例的菜单项回调依赖全局 AppState，嵌入模式下先注册。
        cx.set_global(AppState::new());
        cx.new(|_| SetMenus {}).into()
    }
}

mod tab_stop {

    //! Tab 焦点遍历示例：展示 `tab_stop` 顺序与可聚焦元素。
    //! 用法：左侧 tab 选「tab_stop」，连续按 Tab 观察焦点按序移动。
    use rgpui::{
        App, Bounds, Context, Div, ElementId, FocusHandle, KeyBinding, SharedString, Stateful,
        Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
    };
    use rgpui_platform::application;

    actions!(tab_stop_example, [Tab, TabPrev]);

    struct Example {
        focus_handle: FocusHandle,
        items: Vec<FocusHandle>,
        message: SharedString,
    }

    impl Example {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let items = vec![
                cx.focus_handle().tab_index(1).tab_stop(true),
                cx.focus_handle().tab_index(2).tab_stop(true),
                cx.focus_handle().tab_index(3).tab_stop(true),
                cx.focus_handle(),
                cx.focus_handle().tab_index(2).tab_stop(true),
            ];

            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);

            Self {
                focus_handle,
                items,
                message: SharedString::from("Press `Tab`, `Shift-Tab` to switch focus."),
            }
        }

        fn on_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_next(cx);
            self.message = SharedString::from("You have pressed `Tab`.");
        }

        fn on_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
            window.focus_prev(cx);
            self.message = SharedString::from("You have pressed `Shift-Tab`.");
        }
    }

    impl Render for Example {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            fn tab_stop_style<T: Styled>(this: T) -> T {
                this.border_3().border_color(rgpui::blue())
            }

            fn button(id: impl Into<ElementId>) -> Stateful<Div> {
                div()
                    .id(id)
                    .h_10()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .items_center()
                    .border_1()
                    .border_color(rgpui::black())
                    .bg(rgpui::black())
                    .text_color(rgpui::white())
                    .focus(tab_stop_style)
                    .shadow_sm()
            }

            div()
                .id("app")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_tab))
                .on_action(cx.listener(Self::on_tab_prev))
                .size_full()
                .flex()
                .flex_col()
                .p_4()
                .gap_3()
                .bg(rgpui::white())
                .text_color(rgpui::black())
                .child(self.message.clone())
                .children(
                    self.items
                        .clone()
                        .into_iter()
                        .enumerate()
                        .map(|(ix, item_handle)| {
                            div()
                                .id(("item", ix))
                                .track_focus(&item_handle)
                                .h_10()
                                .w_full()
                                .flex()
                                .justify_center()
                                .items_center()
                                .border_1()
                                .border_color(rgpui::black())
                                .when(
                                    item_handle.tab_stop && item_handle.is_focused(window),
                                    tab_stop_style,
                                )
                                .map(|this| match item_handle.tab_stop {
                                    true => this
                                        .hover(|this| this.bg(rgpui::black().opacity(0.1)))
                                        .child(format!("tab_index: {}", item_handle.tab_index)),
                                    false => this.opacity(0.4).child("tab_stop: false"),
                                })
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_3()
                        .items_center()
                        .child(
                            button("el1")
                                .tab_index(4)
                                .child("Button 1")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message = "You have clicked Button 1.".into();
                                    cx.notify();
                                })),
                        )
                        .child(
                            button("el2")
                                .tab_index(5)
                                .child("Button 2")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message = "You have clicked Button 2.".into();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .id("group-1")
                        .tab_index(6)
                        .tab_group()
                        .tab_stop(false)
                        .child(
                            button("group-1-button-1")
                                .tab_index(1)
                                .child("Tab index [6, 1]"),
                        )
                        .child(
                            button("group-1-button-2")
                                .tab_index(2)
                                .child("Tab index [6, 2]"),
                        )
                        .child(
                            button("group-1-button-3")
                                .tab_index(3)
                                .child("Tab index [6, 3]"),
                        ),
                )
                .child(
                    div()
                        .id("group-2")
                        .tab_index(7)
                        .tab_group()
                        .tab_stop(false)
                        .child(
                            button("group-2-button-1")
                                .tab_index(1)
                                .child("Tab index [7, 1]"),
                        )
                        .child(
                            button("group-2-button-2")
                                .tab_index(2)
                                .child("Tab index [7, 2]"),
                        )
                        .child(
                            button("group-2-button-3")
                                .tab_index(3)
                                .child("Tab index [7, 3]"),
                        ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            cx.bind_keys([
                KeyBinding::new("tab", Tab, None),
                KeyBinding::new("shift-tab", TabPrev, None),
            ]);

            let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Example::new(window, cx)),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| Example::new(window, cx)).into()
    }
}

mod chat {
    //! 聊天三件演示：Bubble 气泡 + MessageScroller 吸底/未读 + Marker 分隔线。

    use rgpui::{
        App, Avatar, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
        chat::{
            Bubble, BubbleSide, Marker, Message, MessageGroup, MessageScroller,
            MessageScrollerState,
        },
        div, h_flex,
        prelude::*,
        px, size, v_flex,
    };
    use rgpui_platform::application;

    struct ChatDemo {
        scroller: rgpui::Entity<MessageScrollerState>,
        groups: Vec<MessageGroup>,
        counter: usize,
    }

    impl ChatDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let scroller = cx.new(|_| MessageScrollerState::new());
            let _ = window;
            Self {
                scroller,
                groups: vec![MessageGroup {
                    sender: "助手".into(),
                    messages: vec![Message::text("你好，有什么可以帮你？")],
                }],
                counter: 0,
            }
        }

        fn push(&mut self, sender: &str, mine: bool, cx: &mut Context<Self>) {
            self.counter += 1;
            let text = if mine {
                format!("我的第 {} 条消息", self.counter)
            } else {
                format!("助手的第 {} 条回复", self.counter)
            };
            match self.groups.last_mut() {
                Some(group) if group.sender.as_str() == sender => {
                    group.messages.push(Message::text(text));
                }
                _ => self.groups.push(MessageGroup {
                    sender: sender.into(),
                    messages: vec![Message::text(text)],
                }),
            }
            cx.notify();
        }
    }

    impl Render for ChatDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let groups = self.groups.clone();
            v_flex()
                .size_full()
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .child(
                            rgpui::Button::new("chat-push-theirs")
                                .label("追加助手消息")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.push("助手", false, cx);
                                })),
                        )
                        .child(
                            rgpui::Button::new("chat-push-mine")
                                .label("追加我的消息")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.push("我", true, cx);
                                })),
                        )
                        .child(rgpui::Button::new("chat-clear").label("清空").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.groups.clear();
                                this.counter = 0;
                                cx.notify();
                            }),
                        )),
                )
                .child(
                    div().flex_1().p(px(12.0)).child(
                        MessageScroller::new(self.scroller.clone()).child(
                            v_flex()
                                .gap(px(4.0))
                                .child(Marker::new("今天 09:00"))
                                .children(groups.into_iter().map(|group| {
                                    let mine = group.sender.as_str() == "我";
                                    let sender = group.sender.clone();
                                    let initial: String =
                                        sender.chars().next().unwrap_or('?').to_string();
                                    v_flex()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .w_full()
                                                .when(mine, |this| this.justify_end())
                                                .when(!mine, |this| this.justify_start())
                                                .child(div().text_xs().px(px(4.0)).child(sender)),
                                        )
                                        .children(group.messages.into_iter().map(move |msg| {
                                            let text = match msg.content {
                                                MessageType::Text(text) => text,
                                                MessageType::CodeBlock { code, .. } => code,
                                            };
                                            Bubble::new()
                                                .side(if mine {
                                                    BubbleSide::Right
                                                } else {
                                                    BubbleSide::Left
                                                })
                                                .avatar(Avatar::new(initial.clone()).size(px(28.0)))
                                                .child(div().text_sm().child(text))
                                        }))
                                })),
                        ),
                    ),
                )
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(560.0), px(680.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| ChatDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| ChatDemo::new(window, cx)).into()
    }
}

mod dock {
    //! Dock 布局演示：四区域标签页、拖拽跨区、关闭/显隐、布局持久化。

    use rgpui::{
        App, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
        components::{DockArea, DockAreaState, DockPanel, DockPosition},
        div, h_flex,
        prelude::*,
        px, size, v_flex,
    };
    use rgpui_platform::application;

    struct DockDemo {
        dock: rgpui::Entity<DockAreaState>,
        saved_len: usize,
    }

    impl DockDemo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let dock = cx.new(|_| DockAreaState::new());
            dock.update(cx, |dock, cx| {
                dock.add_panel(
                    DockPanel::new("files", "文件树", DockPosition::Left, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("src/\n  main.rs\n  lib.rs")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("outline", "大纲", DockPosition::Left, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("fn main\nfn render")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("editor", "编辑器", DockPosition::Center, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("fn main() {\n    println!(\"hello\");\n}")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("props", "属性", DockPosition::Right, |_, _| {
                        div()
                            .p(px(12.0))
                            .child("width: 240\nheight: 自适应")
                            .into_any_element()
                    }),
                    cx,
                );
                dock.add_panel(
                    DockPanel::new("terminal", "终端", DockPosition::Bottom, |_, _| {
                        div().p(px(12.0)).child("$ cargo run").into_any_element()
                    }),
                    cx,
                );
            });
            let _ = window;
            Self { dock, saved_len: 0 }
        }
    }

    impl Render for DockDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let saved = self.saved_len;
            v_flex()
                .size_full()
                .child(
                    h_flex()
                        .gap(px(8.0))
                        .p(px(8.0))
                        .child(rgpui::Button::new("dock-save").label("保存布局").on_click(
                            cx.listener(|this, _, _, cx| {
                                let json = this.dock.read(cx).layout_json();
                                this.saved_len = json.len();
                                cx.notify();
                            }),
                        ))
                        .child(
                            rgpui::Button::new("dock-hide-right")
                                .label("显隐右侧")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let visible =
                                        this.dock.read(cx).is_visible(DockPosition::Right);
                                    this.dock.update(cx, |dock, cx| {
                                        dock.set_visible(DockPosition::Right, !visible, cx);
                                    });
                                })),
                        )
                        .child(div().text_sm().child(format!("已保存布局 {saved} 字节"))),
                )
                .child(div().flex_1().child(DockArea::new(self.dock.clone())))
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| DockDemo::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| DockDemo::new(window, cx)).into()
    }
}

mod grid_layout {

    //! 网格布局示例（圣杯布局）：展示弹性布局中多栏组合。
    //! 用法：左侧 tab 选「grid_layout」，查看多栏自适应排布。
    use rgpui::{
        App, Bounds, Context, Hsla, Window, WindowBounds, WindowOptions, container_query, div,
        prelude::*, px, rgb, size,
    };
    use rgpui_platform::application;

    // https://en.wikipedia.org/wiki/Holy_grail_(web_design)
    //
    // Resize the window: the layout is chosen by `container_query` based on the
    // measured size of the container, collapsing to a single stacked column when
    // it becomes too narrow for the three-column grid.
    struct HolyGrailExample {}

    impl Render for HolyGrailExample {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            container_query(|container_size, _window, _cx| {
                let block = |color: Hsla| {
                    div()
                        .size_full()
                        .bg(color)
                        .border_1()
                        .border_dashed()
                        .rounded_md()
                        .border_color(rgpui::white())
                        .items_center()
                };

                let header =
                    block(rgpui::white()).child(format!("Header  — {}", container_size.width));
                let table_of_contents = block(rgpui::red()).child("Table of contents");
                let content = block(rgpui::green()).child("Content");
                let ad = block(rgpui::blue())
                    .child("AD :(")
                    .text_color(rgpui::white());
                let footer = block(rgpui::black())
                    .text_color(rgpui::white())
                    .child("Footer");

                let container = div().gap_1().bg(rgb(0x505050)).shadow_lg().size_full();

                if container_size.width < px(400.) {
                    container
                        .flex()
                        .flex_col()
                        .child(header.h_12().flex_none())
                        .child(table_of_contents.h_20().flex_none())
                        .child(content.flex_1())
                        .child(ad.h_20().flex_none())
                        .child(footer.h_12().flex_none())
                } else {
                    container
                        .grid()
                        .grid_cols(5)
                        .grid_rows(5)
                        .child(header.row_span(1).col_span_full())
                        .child(table_of_contents.col_span(1).h_56())
                        .child(content.col_span(3).row_span(3))
                        .child(ad.col_span(1).row_span(3))
                        .child(footer.row_span(1).col_span_full())
                }
            })
        }
    }

    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| HolyGrailExample {}),
            )
            .unwrap();
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_| HolyGrailExample {}).into()
    }
}

mod status_bar {
    //! 状态栏（StatusBar）示例。
    //!
    //! `StatusBar` 是纯值驱动的组件：左/右两组 `StatusBarItem`，每项可带图标、提示、
    //! 点击回调、置灰（muted）/激活（active）/禁用（disabled）等状态。每次重绘按当前
    //! 状态重新组装即可，无需同步。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin layout_gallery -- status_bar
    //! ```

    use rgpui::components::status_bar::{StatusBar, StatusBarItem};
    use rgpui::{
        App, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
        WindowOptions, div, h_flex, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 状态栏演示状态（纯值）。
    struct StatusBarDemo {
        line: usize,
        column: usize,
        language: &'static str,
        lsp_connected: bool,
        errors: usize,
        warnings: usize,
        branch: &'static str,
    }

    impl StatusBarDemo {
        fn new() -> Self {
            Self {
                line: 42,
                column: 15,
                language: "Rust",
                lsp_connected: true,
                errors: 2,
                warnings: 5,
                branch: "main",
            }
        }

        /// 按当前值组装状态栏。
        fn status_bar(&self) -> StatusBar {
            let lsp = if self.lsp_connected {
                "rust-analyzer ✓".to_string()
            } else {
                "No LSP".to_string()
            };
            let mut left = vec![StatusBarItem::new(lsp).id("lsp").active(self.lsp_connected)];
            if self.errors > 0 {
                left.push(StatusBarItem::new(format!("{} errors", self.errors)).id("errors"));
            }
            if self.warnings > 0 {
                left.push(StatusBarItem::new(format!("{} warnings", self.warnings)).id("warnings"));
            }
            let right = vec![
                StatusBarItem::new(self.branch).id("branch").muted(true),
                StatusBarItem::new(format!("Ln {}, Col {}", self.line, self.column))
                    .id("cursor")
                    .muted(true),
                StatusBarItem::new(self.language).id("language").muted(true),
                StatusBarItem::new("UTF-8").id("encoding").muted(true),
            ];
            StatusBar::new().left(left).right(right)
        }
    }

    impl Render for StatusBarDemo {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let status_bar = self.status_bar();

            v_flex()
                .id("status-bar-demo")
                .size_full()
                .bg(rgb(0xffffff))
                .child(
                    v_flex()
                        .flex_1()
                        .gap(px(16.0))
                        .p(px(24.0))
                        .child(div().text_2xl().child("状态栏（StatusBar）"))
                        .child(
                            h_flex()
                                .gap(px(8.0))
                                .flex_wrap()
                                .child(button("sb-line-up", "行号 +1", cx, |this, cx| {
                                    this.line += 1;
                                    cx.notify();
                                }))
                                .child(button("sb-col-up", "列号 +5", cx, |this, cx| {
                                    this.column += 5;
                                    cx.notify();
                                }))
                                .child(button("sb-lang", "切换语言", cx, |this, cx| {
                                    this.language = if this.language == "Rust" {
                                        "TypeScript"
                                    } else {
                                        "Rust"
                                    };
                                    cx.notify();
                                }))
                                .child(button("sb-lsp", "切换 LSP", cx, |this, cx| {
                                    this.lsp_connected = !this.lsp_connected;
                                    cx.notify();
                                }))
                                .child(button("sb-error", "错误 +1", cx, |this, cx| {
                                    this.errors += 1;
                                    cx.notify();
                                }))
                                .child(button("sb-reset", "重置", cx, |this, cx| {
                                    *this = StatusBarDemo::new();
                                    cx.notify();
                                })),
                        ),
                )
                // 底部状态栏。
                .child(status_bar)
        }
    }

    /// 一个简单的按钮辅助函数。
    fn button(
        id: &'static str,
        label: &'static str,
        cx: &mut Context<StatusBarDemo>,
        on_click: impl Fn(&mut StatusBarDemo, &mut Context<StatusBarDemo>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px(px(12.0))
            .py(px(6.0))
            .rounded_md()
            .bg(rgb(0xf0f0f0))
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    /// 启动示例。
    pub fn run_example() {
        application().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(720.0), px(420.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| cx.new(|_cx| StatusBarDemo::new()),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(_window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|_cx| StatusBarDemo::new()).into()
    }
}

mod tree {
    //! 树（Tree）示例。
    //!
    //! 演示 `TreeState` 实体 + `tree` 视图：层级展开、点击选中、回车确认。
    //! 通过订阅 `TreeEvent::{Selected, Confirmed}` 拿到选中/确认的节点 id。
    //!
    //! 运行：
    //!
    //! ```text
    //! cargo run -p showcase --bin layout_gallery -- tree
    //! ```

    use rgpui::tree::{TreeEvent, TreeItem, TreeState, tree};
    use rgpui::{
        App, Bounds, Context, Entity, IntoElement, ParentElement, Render, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size, v_flex,
    };
    use rgpui_platform::application;

    /// 示例根视图。
    struct TreeDemo {
        /// 树状态实体。
        tree: Entity<TreeState>,
        /// 事件回显文本。
        message: String,
    }

    impl TreeDemo {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let tree = cx.new(|cx| {
                TreeState::new(cx).items(vec![
                    TreeItem::new("src", "src")
                        .child(TreeItem::new("src/lib.rs", "lib.rs"))
                        .child(TreeItem::new("src/main.rs", "main.rs"))
                        .child(
                            TreeItem::new("src/components", "components")
                                .child(TreeItem::new("src/components/button.rs", "button.rs")),
                        ),
                    TreeItem::new("Cargo.toml", "Cargo.toml"),
                    TreeItem::new("README.md", "README.md"),
                ])
            });
            // 订阅选中 / 确认事件。
            cx.subscribe(&tree, |this, _state, event: &TreeEvent, cx| match event {
                TreeEvent::Selected(id) => {
                    this.message = format!("点击选中：{id}");
                    cx.notify();
                }
                TreeEvent::Confirmed(id) => {
                    this.message = format!("回车确认：{id}");
                    cx.notify();
                }
                _ => {}
            })
            .detach();

            Self {
                tree,
                message: "点击树行选中，回车确认当前行".to_string(),
            }
        }
    }

    impl Render for TreeDemo {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            v_flex()
                .id("tree-demo")
                .size_full()
                .gap(px(16.0))
                .p(px(24.0))
                .bg(rgb(0xffffff))
                .child(div().text_2xl().child("树（Tree）"))
                .child(
                    v_flex()
                        .gap(px(8.0))
                        .p(px(12.0))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0xe0e0e0))
                        .child(tree(&self.tree, |ix, entry, _selected, _window, _cx| {
                            rgpui::list::ListItem::new(("tree-demo", ix))
                                .child(entry.item().label.clone())
                        }))
                        .child(div().text_sm().child(self.message.clone())),
                )
        }
    }

    /// 启动示例：注册树的键绑定。
    pub fn run_example() {
        application().run(|cx: &mut App| {
            rgpui::tree::init(cx);
            let bounds = Bounds::centered(None, size(px(560.0), px(520.0)), cx);
            let _ = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TreeDemo::new(window, cx)),
            );
            cx.activate(true);
        });
    }

    /// 构建可嵌入 gallery 的示例视图。
    pub fn build(window: &mut rgpui::Window, cx: &mut rgpui::App) -> rgpui::AnyView {
        cx.new(|cx| TreeDemo::new(window, cx)).into()
    }
}

/// Gallery 根视图：左侧竖行 tab + 右侧示例内容。
struct GalleryApp {
    titles: Vec<&'static str>,
    /// 当前示例的介绍与用法。
    active: usize,
    views: Vec<AnyView>,
}

impl GalleryApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let titles: Vec<&'static str> = vec![
            "ink",
            "sorter",
            "timeline",
            "color_picker",
            "mdi",
            "task_pane",
            "canvas_a11y",
            "page_view",
            "drag_drop",
            "focus_visible",
            "mouse_pressure",
            "native_menu",
            "native_notification",
            "set_menus",
            "tab_stop",
            "chat",
            "dock",
            "grid_layout",
            "status_bar",
            "tree",
        ];
        let builds: &[fn(&mut Window, &mut App) -> AnyView] = &[
            ink::build,
            sorter::build,
            timeline::build,
            color_picker::build,
            mdi::build,
            task_pane::build,
            canvas_a11y::build,
            page_view::build,
            drag_drop::build,
            focus_visible::build,
            mouse_pressure::build,
            native_menu::build,
            native_notification::build,
            set_menus::build,
            tab_stop::build,
            chat::build,
            dock::build,
            grid_layout::build,
            status_bar::build,
            tree::build,
        ];
        let views = builds.iter().map(|build| build(window, cx)).collect();
        Self {
            titles,
            active: 0,
            views,
        }
    }
}

impl Render for GalleryApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let theme = cx.theme();
        div()
            .id("gallery-app")
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .id("gallery-tabs")
                    .h_full()
                    .w(px(180.0))
                    .flex_shrink_0()
                    .flex_col()
                    .overflow_y_scroll()
                    .border_r_1()
                    .border_color(theme.border)
                    .children(self.titles.iter().enumerate().map(|(ix, title)| {
                        let this = this.clone();
                        Button::new(("gallery-tab", ix))
                            .label(*title)
                            .w_full()
                            .when(ix == self.active, |b| b.primary())
                            .when(ix != self.active, |b| b.ghost())
                            .on_click(move |_, _, cx| {
                                this.update(cx, |gallery, cx| {
                                    gallery.active = ix;
                                    cx.notify();
                                });
                            })
                    })),
            )
            .child(
                div()
                    .id("gallery-content")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.views[self.active].clone()),
            )
    }
}

fn run() {
    // 传 -- <demo> 时保留原有独立窗口行为；不带参数时打开 gallery 导航界面。
    if let Some(name) = std::env::args().nth(1) {
        match name.as_str() {
            "ink" => ink::run_example(),
            "sorter" => sorter::run_example(),
            "timeline" => timeline::run_example(),
            "color_picker" => color_picker::run_example(),
            "mdi" => mdi::run_example(),
            "task_pane" => task_pane::run_example(),
            "canvas_a11y" => canvas_a11y::run_example(),
            "page_view" => page_view::run_example(),
            "drag_drop" => drag_drop::run_example(),
            "focus_visible" => focus_visible::run_example(),
            "mouse_pressure" => mouse_pressure::run_example(),
            "native_menu" => native_menu::run_example(),
            "native_notification" => native_notification::run_example(),
            "set_menus" => set_menus::run_example(),
            "tab_stop" => tab_stop::run_example(),
            "chat" => chat::run_example(),
            "dock" => dock::run_example(),
            "grid_layout" => grid_layout::run_example(),
            "status_bar" => status_bar::run_example(),
            "tree" => tree::run_example(),
            _ => eprintln!("未知 demo：{name}"),
        }
        return;
    }
    application().run(|cx: &mut App| {
        rgpui::theme::init(cx);
        rgpui::input_ui::init(cx);
        rgpui::menu::init(cx);
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| GalleryApp::new(window, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rgpui_platform::web_init();
    run();
}
