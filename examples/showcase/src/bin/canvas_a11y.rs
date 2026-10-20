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
    AccessibleAction, App, BorderStyle, Bounds, Button, Context, Corners, Edges, FocusHandle, Hsla,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, Render,
    Role, SharedString, Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, canvas,
    div, point, prelude::*, px, quad, size, transparent_black,
};
use rgpui_platform::application;

/// 卡片圆角半径。
const CARD_RADIUS: Pixels = px(12.0);
/// 焦点 / 选中光晕的外扩量。
const HALO: Pixels = px(5.0);

/// 画布对象：自绘卡片（或圆形），不带任何子元素。
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

/// 演示视图。
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
                    format!("无障碍点击：{}（{}）", self.label_of(id), self.value_of(id)).into();
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

/// 局部矩形 → 窗口矩形。
fn translate(bounds: Bounds<Pixels>, origin: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(bounds.origin.x + origin.x, bounds.origin.y + origin.y),
        bounds.size,
    )
}

/// 向外扩张矩形（画光晕用）。
fn inflate(bounds: Bounds<Pixels>, by: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        point(bounds.origin.x - by, bounds.origin.y - by),
        size(bounds.size.width + by * 2.0, bounds.size.height + by * 2.0),
    )
}

/// 绘制画布内容：先铺焦点 / 选中光晕，再画对象本体。
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

fn run_example() {
    application().run(|cx: &mut App| {
        rgpui::init_all(cx);
        let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Canvas Accessibility Example".into()),
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
