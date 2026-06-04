use crate::{
    Bounds, Capslock, Context, Empty, IntoElement, Keystroke, Modifiers, Pixels, Point, Render,
    Window, point, seal::Sealed,
};
use smallvec::SmallVec;
use std::{any::Any, fmt::Debug, ops::Deref, path::PathBuf};

/// 来自平台输入源的事件 trait。
///
/// 所有平台输入事件（键盘、鼠标、触摸等）都实现此 trait，
/// 提供统一的事件处理接口。
pub trait InputEvent: Sealed + 'static {
    /// 将此事件转换为平台输入枚举 [`PlatformInput`]
    fn to_platform_input(self) -> PlatformInput;
}

/// 来自平台的按键事件标记 trait。
pub trait KeyEvent: InputEvent {}

/// 来自平台的鼠标事件标记 trait。
pub trait MouseEvent: InputEvent {}

/// 来自平台的手势事件标记 trait。
pub trait GestureEvent: InputEvent {}

/// 平台的按键按下事件。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyDownEvent {
    /// 生成的按键序列（包含修饰键状态和按键字符）
    pub keystroke: Keystroke,

    /// 按键当前是否处于长按状态（auto-repeat）
    pub is_held: bool,

    /// 对于此按键序列，是否优先使用字符输入而非键绑定。
    /// 在某些情况下（如 Windows 上的 AltGr），修饰键对字符输入很重要。
    pub prefer_character_input: bool,
}

impl Sealed for KeyDownEvent {}
impl InputEvent for KeyDownEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::KeyDown(self)
    }
}
impl KeyEvent for KeyDownEvent {}

/// 平台的按键释放事件。
#[derive(Clone, Debug)]
pub struct KeyUpEvent {
    /// 被释放的按键序列
    pub keystroke: Keystroke,
}

impl Sealed for KeyUpEvent {}
impl InputEvent for KeyUpEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::KeyUp(self)
    }
}
impl KeyEvent for KeyUpEvent {}

/// 平台的修饰键状态变化事件。
///
/// 当 Ctrl、Shift、Alt、Cmd 等修饰键被按下或释放时触发。
#[derive(Clone, Debug, Default)]
pub struct ModifiersChangedEvent {
    /// 修饰键的新状态（哪些修饰键被按下）
    pub modifiers: Modifiers,
    /// 大写锁定键的新状态
    pub capslock: Capslock,
}

impl Sealed for ModifiersChangedEvent {}
impl InputEvent for ModifiersChangedEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::ModifiersChanged(self)
    }
}
impl KeyEvent for ModifiersChangedEvent {}

impl Deref for ModifiersChangedEvent {
    type Target = Modifiers;

    fn deref(&self) -> &Self::Target {
        &self.modifiers
    }
}

/// 触摸运动事件的阶段。
/// 基于同名 winit 枚举。
#[derive(Clone, Copy, Debug, Default)]
pub enum TouchPhase {
    /// 触摸开始。
    Started,
    /// 触摸事件正在移动。
    #[default]
    Moved,
    /// 触摸阶段已结束
    Ended,
}

/// 来自平台的鼠标按下事件
#[derive(Clone, Debug, Default)]
pub struct MouseDownEvent {
    /// 哪个鼠标按钮被按下。
    pub button: MouseButton,

    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 鼠标按下时按住的修饰符。
    pub modifiers: Modifiers,

    /// 按钮已被点击的次数。
    pub click_count: usize,

    /// 这是否是第一次聚焦点击。
    pub first_mouse: bool,
}

impl Sealed for MouseDownEvent {}
impl InputEvent for MouseDownEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::MouseDown(self)
    }
}
impl MouseEvent for MouseDownEvent {}

impl MouseDownEvent {
    /// 如果此鼠标释放事件应该聚焦元素则返回 true。
    pub fn is_focusing(&self) -> bool {
        match self.button {
            MouseButton::Left => true,
            _ => false,
        }
    }
}

/// 来自平台的鼠标释放事件
#[derive(Clone, Debug, Default)]
pub struct MouseUpEvent {
    /// 哪个鼠标按钮被释放。
    pub button: MouseButton,

    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 鼠标释放时按住的修饰符。
    pub modifiers: Modifiers,

    /// 按钮已被点击的次数。
    pub click_count: usize,
}

impl Sealed for MouseUpEvent {}
impl InputEvent for MouseUpEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::MouseUp(self)
    }
}

impl MouseEvent for MouseUpEvent {}

impl MouseUpEvent {
    /// 如果此鼠标释放事件应该聚焦元素则返回 true。
    pub fn is_focusing(&self) -> bool {
        match self.button {
            MouseButton::Left => true,
            _ => false,
        }
    }
}

/// 点击事件，当鼠标按钮按下并释放时生成。
#[derive(Clone, Debug, Default)]
pub struct MouseClickEvent {
    /// 按钮按下时的鼠标事件。
    pub down: MouseDownEvent,

    /// 按钮释放时的鼠标事件。
    pub up: MouseUpEvent,
}

/// 压力点击事件的阶段。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PressureStage {
    /// 无压力。
    #[default]
    Zero,
    /// 正常点击压力。
    Normal,
    /// 高压力，足以触发强制点击。
    Force,
}

/// 来自平台的鼠标压力事件。当用力按压力敏感触控板时生成。
/// 目前仅在 macOS 触控板上实现。
#[derive(Debug, Clone, Default)]
pub struct MousePressureEvent {
    /// 当前阶段的压力值，范围为 0 到 1 的浮点数
    pub pressure: f32,
    /// 事件的压力阶段。
    pub stage: PressureStage,
    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,
    /// 鼠标压力更改时按住的修饰符。
    pub modifiers: Modifiers,
}

impl Sealed for MousePressureEvent {}
impl InputEvent for MousePressureEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::MousePressure(self)
    }
}
impl MouseEvent for MousePressureEvent {}

/// 由键盘按钮按下并释放生成的点击事件。
#[derive(Clone, Debug, Default)]
pub struct KeyboardClickEvent {
    /// 触发点击的键盘按钮。
    pub button: KeyboardButton,

    /// 被点击元素的边界。
    pub bounds: Bounds<Pixels>,
}

/// 点击事件，当鼠标按钮或键盘按钮按下并释放时生成。
#[derive(Clone, Debug)]
pub enum ClickEvent {
    /// 由鼠标按钮按下并释放触发的点击事件。
    Mouse(MouseClickEvent),
    /// 由键盘按钮按下并释放触发的点击事件。
    Keyboard(KeyboardClickEvent),
}

impl Default for ClickEvent {
    fn default() -> Self {
        ClickEvent::Keyboard(KeyboardClickEvent::default())
    }
}

impl ClickEvent {
    /// 返回点击事件期间按住的修饰符
    ///
    /// `Keyboard`：键盘点击事件永远不会带有修饰符。
    /// `Mouse`：鼠标按键释放事件期间按住的修饰符。
    pub fn modifiers(&self) -> Modifiers {
        match self {
            // 点击事件仅由不带任何修饰符的键盘事件生成，因此我们知道修饰符始终为默认值
            ClickEvent::Keyboard(_) => Modifiers::default(),
            // 在 Web 上的点击事件仅反映按键释放事件的修饰符，
            // 通过在 Chrome 138 中观察 `ClickEvent.shiftKey` 字段在不同修饰符和按键释放/按下事件组合下的行为进行了测试。
            ClickEvent::Mouse(event) => event.up.modifiers,
        }
    }

    /// 返回点击事件的位置
    ///
    /// `Keyboard`：被点击命中框的左下角
    /// `Mouse`：按钮释放时鼠标的位置。
    pub fn position(&self) -> Point<Pixels> {
        match self {
            ClickEvent::Keyboard(event) => event.bounds.bottom_left(),
            ClickEvent::Mouse(event) => event.up.position,
        }
    }

    /// 返回点击事件的鼠标位置
    ///
    /// `Keyboard`：None
    /// `Mouse`：按钮释放时鼠标的位置。
    pub fn mouse_position(&self) -> Option<Point<Pixels>> {
        match self {
            ClickEvent::Keyboard(_) => None,
            ClickEvent::Mouse(event) => Some(event.up.position),
        }
    }

    /// 返回这是否是右键点击
    ///
    /// `Keyboard`：false
    /// `Mouse`：是否按下并释放了右键
    pub fn is_right_click(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => false,
            ClickEvent::Mouse(event) => {
                event.down.button == MouseButton::Right && event.up.button == MouseButton::Right
            }
        }
    }

    /// 返回这是否是中键点击
    ///
    /// `Keyboard`：false
    /// `Mouse`：是否按下并释放了中键
    pub fn is_middle_click(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => false,
            ClickEvent::Mouse(event) => {
                event.down.button == MouseButton::Middle && event.up.button == MouseButton::Middle
            }
        }
    }

    /// 返回点击是否是标准点击
    ///
    /// `Keyboard`：始终为 true
    /// `Mouse`：左键按下并释放
    pub fn standard_click(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => true,
            ClickEvent::Mouse(event) => {
                event.down.button == MouseButton::Left && event.up.button == MouseButton::Left
            }
        }
    }

    /// 返回点击是否聚焦了元素
    ///
    /// `Keyboard`：false，键盘点击仅在元素已聚焦时有效
    /// `Mouse`：这是否是第一次聚焦点击
    pub fn first_focus(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => false,
            ClickEvent::Mouse(event) => event.down.first_mouse,
        }
    }

    /// 返回点击事件的点击次数
    ///
    /// `Keyboard`：始终为 1
    /// `Mouse`：MouseUpEvent 的点击次数
    pub fn click_count(&self) -> usize {
        match self {
            ClickEvent::Keyboard(_) => 1,
            ClickEvent::Mouse(event) => event.up.click_count,
        }
    }

    /// 返回点击事件是否由键盘事件生成
    pub fn is_keyboard(&self) -> bool {
        match self {
            ClickEvent::Mouse(_) => false,
            ClickEvent::Keyboard(_) => true,
        }
    }
}

/// 表示点击事件按下的键盘按钮的枚举。
#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Default)]
pub enum KeyboardButton {
    /// 回车键被点击
    #[default]
    Enter,
    /// 空格键被点击
    Space,
}

/// 表示按下的鼠标按钮的枚举。
#[derive(Hash, Default, PartialEq, Eq, Copy, Clone, Debug)]
pub enum MouseButton {
    /// 鼠标左键。
    #[default]
    Left,

    /// 鼠标右键。
    Right,

    /// 鼠标中键。
    Middle,

    /// 导航按钮，如后退或前进。
    Navigate(NavigationDirection),
}

impl MouseButton {
    /// 获取列表中的所有鼠标按钮。
    pub fn all() -> Vec<Self> {
        vec![
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Navigate(NavigationDirection::Back),
            MouseButton::Navigate(NavigationDirection::Forward),
        ]
    }
}

/// 导航方向，如后退或前进。
#[derive(Hash, Default, PartialEq, Eq, Copy, Clone, Debug)]
pub enum NavigationDirection {
    /// 后退按钮。
    #[default]
    Back,

    /// 前进按钮。
    Forward,
}

/// 来自平台的鼠标移动事件。
#[derive(Clone, Debug, Default)]
pub struct MouseMoveEvent {
    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 按下的鼠标按钮（如果有）。
    pub pressed_button: Option<MouseButton>,

    /// 鼠标移动时按住的修饰符。
    pub modifiers: Modifiers,
}

impl Sealed for MouseMoveEvent {}
impl InputEvent for MouseMoveEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::MouseMove(self)
    }
}
impl MouseEvent for MouseMoveEvent {}

impl MouseMoveEvent {
    /// 如果左鼠标按钮当前被按住则返回 true。
    pub fn dragging(&self) -> bool {
        self.pressed_button == Some(MouseButton::Left)
    }
}

/// 来自平台的鼠标滚轮事件。
#[derive(Clone, Debug, Default)]
pub struct ScrollWheelEvent {
    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 此事件的滚轮位置变化。
    pub delta: ScrollDelta,

    /// 鼠标移动时按住的修饰符。
    pub modifiers: Modifiers,

    /// 触摸事件的阶段。
    pub touch_phase: TouchPhase,
}

impl Sealed for ScrollWheelEvent {}
impl InputEvent for ScrollWheelEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::ScrollWheel(self)
    }
}
impl MouseEvent for ScrollWheelEvent {}

impl Deref for ScrollWheelEvent {
    type Target = Modifiers;

    fn deref(&self) -> &Self::Target {
        &self.modifiers
    }
}

/// 滚轮事件的滚动增量。
#[derive(Clone, Copy, Debug)]
pub enum ScrollDelta {
    /// 以像素为单位的精确滚动增量。
    Pixels(Point<Pixels>),
    /// 以行为单位的不精确滚动增量。
    Lines(Point<f32>),
}

impl Default for ScrollDelta {
    fn default() -> Self {
        Self::Lines(Default::default())
    }
}

/// 来自平台的捏合手势事件，当用户执行
/// 捏合缩放手势时生成（通常在触控板上）。
///
#[derive(Clone, Debug, Default)]
pub struct PinchEvent {
    /// 捏合中心在窗口上的位置。
    pub position: Point<Pixels>,

    /// 此事件的缩放增量。
    /// 正值表示放大，负值表示缩小。
    /// 例如，0.1 表示 10% 的放大。
    pub delta: f32,

    /// 捏合手势期间按住的修饰符。
    pub modifiers: Modifiers,

    /// 捏合手势的阶段。
    pub phase: TouchPhase,
}

impl Sealed for PinchEvent {}
impl InputEvent for PinchEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::Pinch(self)
    }
}
impl GestureEvent for PinchEvent {}
impl MouseEvent for PinchEvent {}

impl Deref for PinchEvent {
    type Target = Modifiers;

    fn deref(&self) -> &Self::Target {
        &self.modifiers
    }
}

impl ScrollDelta {
    /// 如果这是精确的像素滚动增量则返回 true。
    pub fn precise(&self) -> bool {
        match self {
            ScrollDelta::Pixels(_) => true,
            ScrollDelta::Lines(_) => false,
        }
    }

    /// 将此滚动事件转换为精确像素。
    pub fn pixel_delta(&self, line_height: Pixels) -> Point<Pixels> {
        match self {
            ScrollDelta::Pixels(delta) => *delta,
            ScrollDelta::Lines(delta) => point(line_height * delta.x, line_height * delta.y),
        }
    }

    /// 将两个滚动增量合并为一个。
    /// 如果增量的符号相同（都为正或都为负），
    /// 则增量相加。如果符号相反，则使用第二个增量
    ///（other），有效地覆盖第一个增量。
    pub fn coalesce(self, other: ScrollDelta) -> ScrollDelta {
        match (self, other) {
            (ScrollDelta::Pixels(a), ScrollDelta::Pixels(b)) => {
                let x = if a.x.signum() == b.x.signum() {
                    a.x + b.x
                } else {
                    b.x
                };

                let y = if a.y.signum() == b.y.signum() {
                    a.y + b.y
                } else {
                    b.y
                };

                ScrollDelta::Pixels(point(x, y))
            }

            (ScrollDelta::Lines(a), ScrollDelta::Lines(b)) => {
                let x = if a.x.signum() == b.x.signum() {
                    a.x + b.x
                } else {
                    b.x
                };

                let y = if a.y.signum() == b.y.signum() {
                    a.y + b.y
                } else {
                    b.y
                };

                ScrollDelta::Lines(point(x, y))
            }

            _ => other,
        }
    }
}

/// 来自平台的鼠标离开事件，当鼠标离开窗口时生成。
#[derive(Clone, Debug, Default)]
pub struct MouseExitEvent {
    /// 鼠标相对于窗口的位置。
    pub position: Point<Pixels>,
    /// 按下的鼠标按钮（如果有）。
    pub pressed_button: Option<MouseButton>,
    /// 鼠标移动时按住的修饰符。
    pub modifiers: Modifiers,
}

impl Sealed for MouseExitEvent {}
impl InputEvent for MouseExitEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::MouseExited(self)
    }
}

impl MouseEvent for MouseExitEvent {}

impl Deref for MouseExitEvent {
    type Target = Modifiers;

    fn deref(&self) -> &Self::Target {
        &self.modifiers
    }
}

/// 来自平台的路径集合，如从文件拖放中获取。
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ExternalPaths(pub SmallVec<[PathBuf; 2]>);

impl ExternalPaths {
    /// 将此路径集合转换为切片。
    pub fn paths(&self) -> &[PathBuf] {
        &self.0
    }
}

impl Render for ExternalPaths {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // 平台将为拖动的文件显示图标
        Empty
    }
}

/// 来自平台的文件拖放事件，当文件被拖放并放到窗口上时生成。
#[derive(Debug, Clone)]
pub enum FileDropEvent {
    /// 文件已进入窗口。
    Entered {
        /// 鼠标相对于窗口的位置。
        position: Point<Pixels>,
        /// 被拖动的文件路径。
        paths: ExternalPaths,
    },
    /// 文件正在窗口上被拖动
    Pending {
        /// 鼠标相对于窗口的位置。
        position: Point<Pixels>,
    },
    /// 文件已被放到窗口上。
    Submit {
        /// 鼠标相对于窗口的位置。
        position: Point<Pixels>,
    },
    /// 用户已停止在窗口上拖动文件。
    Exited,
}

impl Sealed for FileDropEvent {}
impl InputEvent for FileDropEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::FileDrop(self)
    }
}
impl MouseEvent for FileDropEvent {}

/// 对应所有类型平台输入事件的枚举。
#[derive(Clone, Debug)]
pub enum PlatformInput {
    /// 按键被按下。
    KeyDown(KeyDownEvent),
    /// 按键被释放。
    KeyUp(KeyUpEvent),
    /// 键盘修饰符被更改。
    ModifiersChanged(ModifiersChangedEvent),
    /// 鼠标被按下。
    MouseDown(MouseDownEvent),
    /// 鼠标被释放。
    MouseUp(MouseUpEvent),
    /// 鼠标压力。
    MousePressure(MousePressureEvent),
    /// 鼠标被移动。
    MouseMove(MouseMoveEvent),
    /// 鼠标离开窗口。
    MouseExited(MouseExitEvent),
    /// 滚轮被使用。
    ScrollWheel(ScrollWheelEvent),
    /// 执行了捏合手势。
    Pinch(PinchEvent),
    /// 文件被拖放并放到窗口上。
    FileDrop(FileDropEvent),
}

impl PlatformInput {
    pub(crate) fn mouse_event(&self) -> Option<&dyn Any> {
        match self {
            PlatformInput::KeyDown { .. } => None,
            PlatformInput::KeyUp { .. } => None,
            PlatformInput::ModifiersChanged { .. } => None,
            PlatformInput::MouseDown(event) => Some(event),
            PlatformInput::MouseUp(event) => Some(event),
            PlatformInput::MouseMove(event) => Some(event),
            PlatformInput::MousePressure(event) => Some(event),
            PlatformInput::MouseExited(event) => Some(event),
            PlatformInput::ScrollWheel(event) => Some(event),
            PlatformInput::Pinch(event) => Some(event),
            PlatformInput::FileDrop(event) => Some(event),
        }
    }

    pub(crate) fn keyboard_event(&self) -> Option<&dyn Any> {
        match self {
            PlatformInput::KeyDown(event) => Some(event),
            PlatformInput::KeyUp(event) => Some(event),
            PlatformInput::ModifiersChanged(event) => Some(event),
            PlatformInput::MouseDown(_) => None,
            PlatformInput::MouseUp(_) => None,
            PlatformInput::MouseMove(_) => None,
            PlatformInput::MousePressure(_) => None,
            PlatformInput::MouseExited(_) => None,
            PlatformInput::ScrollWheel(_) => None,
            PlatformInput::Pinch(_) => None,
            PlatformInput::FileDrop(_) => None,
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        self as rgpui, AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement,
        KeyBinding, Keystroke, ParentElement, Render, TestAppContext, Window, div,
    };

    struct TestView {
        saw_key_down: bool,
        saw_action: bool,
        focus_handle: FocusHandle,
    }

    actions!(test_only, [TestAction]);

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().id("testview").child(
                div()
                    .key_context("parent")
                    .on_key_down(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.saw_key_down = true
                    }))
                    .on_action(cx.listener(|this: &mut TestView, _: &TestAction, _, _| {
                        this.saw_action = true
                    }))
                    .child(
                        div()
                            .key_context("nested")
                            .track_focus(&self.focus_handle)
                            .into_element(),
                    ),
            )
        }
    }

    #[rgpui::test]
    fn test_on_events(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| TestView {
                    saw_key_down: false,
                    saw_action: false,
                    focus_handle: cx.focus_handle(),
                })
            })
            .unwrap()
        });

        cx.update(|cx| {
            cx.bind_keys(vec![KeyBinding::new("ctrl-g", TestAction, Some("parent"))]);
        });

        window
            .update(cx, |test_view, window, cx| {
                window.focus(&test_view.focus_handle, cx)
            })
            .unwrap();

        cx.dispatch_keystroke(*window, Keystroke::parse("a").unwrap());
        cx.dispatch_keystroke(*window, Keystroke::parse("ctrl-g").unwrap());

        window
            .update(cx, |test_view, _, _| {
                assert!(test_view.saw_key_down || test_view.saw_action);
                assert!(test_view.saw_key_down);
                assert!(test_view.saw_action);
            })
            .unwrap();
    }
}
