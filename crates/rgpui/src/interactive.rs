use crate::{
    Bounds, Capslock, Context, Empty, IntoElement, Keystroke, Modifiers, Pixels, Point, Render,
    Window, point, seal::Sealed,
};
use smallvec::SmallVec;
use std::{any::Any, fmt::Debug, ops::Deref, path::PathBuf};

/// 来自平台输入源的事件。
pub trait InputEvent: Sealed + 'static {
    /// 将此事件转换为平台输入枚举。
    fn to_platform_input(self) -> PlatformInput;
}

/// 来自平台的按键事件。
pub trait KeyEvent: InputEvent {}

/// 来自平台的鼠标事件。
pub trait MouseEvent: InputEvent {}

/// 来自平台的手势事件。
pub trait GestureEvent: InputEvent {}

/// 平台的按键按下事件等价类型。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyDownEvent {
    /// 生成的按键输入。
    pub keystroke: Keystroke,

    /// 按键当前是否处于按住状态。
    pub is_held: bool,

    /// 是否优先处理字符输入而非按键绑定。
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

/// 平台的按键释放事件等价类型。
#[derive(Clone, Debug)]
pub struct KeyUpEvent {
    /// 释放的按键输入。
    pub keystroke: Keystroke,
}

impl Sealed for KeyUpEvent {}
impl InputEvent for KeyUpEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::KeyUp(self)
    }
}
impl KeyEvent for KeyUpEvent {}

/// 平台的修饰键变更事件等价类型。
#[derive(Clone, Debug, Default)]
pub struct ModifiersChangedEvent {
    /// 修饰键的新状态
    pub modifiers: Modifiers,
    /// Caps Lock 键的新状态
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

/// 触摸移动事件的阶段。
/// 基于 winit 的同名枚举。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TouchPhase {
    /// 触摸开始。
    Started,
    /// 触摸事件正在移动。
    #[default]
    Moved,
    /// 触摸阶段已结束
    Ended,
    /// 触摸被取消：系统接管了该触摸，不会正常结束。
    /// 消费者必须完全回退任何进行中的交互，将触摸视为从未发生。
    Cancelled,
}

/// 标识一个触摸（手指或触控笔接触）的整个生命周期，
/// 从 [`TouchPhase::Started`] 到 [`TouchPhase::Ended`] 或
/// [`TouchPhase::Cancelled`]。
///
/// 该值是不透明的且由平台定义；仅保证在触摸持续期间稳定，
/// 且在并发触摸中唯一。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TouchId(pub u64);

/// 来自平台的原始触摸事件。
///
///
/// 分发契约（核心实现待完成）：触摸仅在 [`TouchPhase::Started`] 时进行
/// 一次命中测试，考虑遮挡；同一 [`TouchId`] 的所有后续事件都发送到
/// 起始位置下的元素，即使触摸已移出该区域。
#[derive(Clone, Debug, Default)]
pub struct TouchEvent {
    /// 此事件所属的触摸。
    pub id: TouchId,
    /// 触摸的阶段。
    pub phase: TouchPhase,
    /// 触摸在窗口坐标中的位置。
    pub position: Point<Pixels>,
    /// 归一化的触摸力度，范围为 `0.0..=1.0`（如果硬件支持报告）。
    pub force: Option<f32>,
}

impl Sealed for TouchEvent {}
impl InputEvent for TouchEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::Touch(self)
    }
}

/// 来自平台的鼠标按下事件
#[derive(Clone, Debug, Default)]
pub struct MouseDownEvent {
    /// 按下的鼠标按键。
    pub button: MouseButton,

    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 鼠标按下时按住的修饰键。
    pub modifiers: Modifiers,

    /// 按钮被点击的次数。
    pub click_count: usize,

    /// 是否是首次聚焦点击。
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
    /// 如果此鼠标按下事件应聚焦元素则返回 true。
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
    /// 释放的鼠标按键。
    pub button: MouseButton,

    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 鼠标释放时按住的修饰键。
    pub modifiers: Modifiers,

    /// 按钮被点击的次数。
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
    /// 如果此鼠标释放事件应聚焦元素则返回 true。
    pub fn is_focusing(&self) -> bool {
        match self.button {
            MouseButton::Left => true,
            _ => false,
        }
    }
}

/// 点击事件，当鼠标按键按下并释放时生成。
#[derive(Clone, Debug, Default)]
pub struct MouseClickEvent {
    /// 按下按钮时的鼠标事件。
    pub down: MouseDownEvent,

    /// 释放按钮时的鼠标事件。
    pub up: MouseUpEvent,
}

/// 压力点击事件的阶段。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PressureStage {
    /// 无压力。
    #[default]
    Zero,
    /// 普通点击压力。
    Normal,
    /// 高压力，足以触发强制点击。
    Force,
}

/// 来自平台的鼠标压力事件。当用力按压力敏触摸板时生成。
/// 目前仅在 macOS 触摸板上实现。
#[derive(Debug, Clone, Default)]
pub struct MousePressureEvent {
    /// 当前阶段的压力，范围为 0 到 1 的浮点数
    pub pressure: f32,
    /// 事件的压力阶段。
    pub stage: PressureStage,
    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 鼠标压力变化时按住的修饰键。
    pub modifiers: Modifiers,
}

impl Sealed for MousePressureEvent {}
impl InputEvent for MousePressureEvent {
    fn to_platform_input(self) -> PlatformInput {
        PlatformInput::MousePressure(self)
    }
}
impl MouseEvent for MousePressureEvent {}

/// 由键盘按键按下并释放生成的点击事件。
#[derive(Clone, Debug, Default)]
pub struct KeyboardClickEvent {
    /// 按下以触发点击的键盘按键。
    pub button: KeyboardButton,

    /// 被点击元素的边界。
    pub bounds: Bounds<Pixels>,
}

/// 由触摸屏上识别的轻触手势生成的点击事件。
#[derive(Clone, Debug, Default)]
pub struct TouchClickEvent {
    /// 轻触在窗口坐标中的位置。
    pub position: Point<Pixels>,
    /// 该位置连续轻触的次数（双击 = 2），
    /// 类似于鼠标的 `click_count`。
    pub tap_count: usize,
    /// 是否是长按而非轻触。长按是触摸的次要激活方式：
    /// 它们与右键点击一起发送到辅助点击监听器，
    /// 而非主点击监听器。
    pub long_press: bool,
}

/// 点击事件，当鼠标按键或键盘按键按下并释放时生成，
/// 或当触摸屏上识别到轻触手势时生成。
#[derive(Clone, Debug)]
pub enum ClickEvent {
    /// 鼠标按键按下并释放触发的点击事件。
    Mouse(MouseClickEvent),
    /// 键盘按键按下并释放触发的点击事件。
    Keyboard(KeyboardClickEvent),
    /// 触摸屏上识别的轻触手势触发的点击事件。
    Touch(TouchClickEvent),
}

impl Default for ClickEvent {
    fn default() -> Self {
        ClickEvent::Keyboard(KeyboardClickEvent::default())
    }
}

impl ClickEvent {
    /// 返回点击事件期间按住的修饰键
    ///
    /// `Keyboard`：键盘点击事件始终没有修饰键。
    /// `Mouse`：鼠标按键释放事件期间按住的修饰键。
    pub fn modifiers(&self) -> Modifiers {
        match self {
            // Click events are only generated from keyboard events _without any modifiers_, so we know the modifiers are always Default
            ClickEvent::Keyboard(_) => Modifiers::default(),
            // Click events on the web only reflect the modifiers for the keyup event,
            // tested via observing the behavior of the `ClickEvent.shiftKey` field in Chrome 138
            // under various combinations of modifiers and keyUp / keyDown events.
            ClickEvent::Mouse(event) => event.up.modifiers,
            // Touch screens have no modifier keys.
            ClickEvent::Touch(_) => Modifiers::default(),
        }
    }

    /// 返回点击事件的位置
    ///
    /// `Keyboard`：被点击碰撞框的左下角
    /// `Mouse`：按钮释放时鼠标的位置。
    /// `Touch`：轻触的位置。
    pub fn position(&self) -> Point<Pixels> {
        match self {
            ClickEvent::Keyboard(event) => event.bounds.bottom_left(),
            ClickEvent::Mouse(event) => event.up.position,
            ClickEvent::Touch(event) => event.position,
        }
    }

    /// 返回点击事件的鼠标位置
    ///
    /// `Keyboard`：None
    /// `Mouse`：按钮释放时鼠标的位置。
    /// `Touch`：None，触摸不是鼠标输入且没有光标。
    pub fn mouse_position(&self) -> Option<Point<Pixels>> {
        match self {
            ClickEvent::Keyboard(_) => None,
            ClickEvent::Mouse(event) => Some(event.up.position),
            ClickEvent::Touch(_) => None,
        }
    }

    /// 返回是否为右键点击
    ///
    /// `Keyboard`：false
    /// `Mouse`：右键是否被按下并释放
    pub fn is_right_click(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => false,
            ClickEvent::Mouse(event) => {
                event.down.button == MouseButton::Right && event.up.button == MouseButton::Right
            }
            ClickEvent::Touch(_) => false,
        }
    }

    /// 返回是否为中键点击
    ///
    /// `Keyboard`：false
    /// `Mouse`：中键是否被按下并释放
    pub fn is_middle_click(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => false,
            ClickEvent::Mouse(event) => {
                event.down.button == MouseButton::Middle && event.up.button == MouseButton::Middle
            }
            ClickEvent::Touch(_) => false,
        }
    }

    /// 返回点击是否为次要激活，即上下文菜单触发器：
    /// 鼠标的右键点击（macOS 的 ctrl-click 已由平台层转换为右键点击），
    /// 或触摸屏上的长按。
    pub fn is_secondary(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => false,
            ClickEvent::Mouse(event) => {
                event.down.button == MouseButton::Right && event.up.button == MouseButton::Right
            }
            ClickEvent::Touch(event) => event.long_press,
        }
    }

    /// 返回点击是否为标准点击
    ///
    /// `Keyboard`：始终为 true
    /// `Mouse`：左键按下并释放
    /// `Touch`：轻触但非长按
    pub fn standard_click(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => true,
            ClickEvent::Mouse(event) => {
                event.down.button == MouseButton::Left && event.up.button == MouseButton::Left
            }
            ClickEvent::Touch(event) => !event.long_press,
        }
    }

    /// 返回点击是否聚焦了元素
    ///
    /// `Keyboard`：false，键盘点击仅在元素已聚焦时有效
    /// `Mouse`：是否是首次聚焦点击
    /// `Touch`：false，移动端窗口在可点击时已处于活动状态
    pub fn first_focus(&self) -> bool {
        match self {
            ClickEvent::Keyboard(_) => false,
            ClickEvent::Mouse(event) => event.down.first_mouse,
            ClickEvent::Touch(_) => false,
        }
    }

    /// 返回点击事件的点击次数
    ///
    /// `Keyboard`：始终为 1
    /// `Mouse`：MouseUpEvent 中的点击计数
    /// `Touch`：连续轻触次数
    pub fn click_count(&self) -> usize {
        match self {
            ClickEvent::Keyboard(_) => 1,
            ClickEvent::Mouse(event) => event.up.click_count,
            ClickEvent::Touch(event) => event.tap_count,
        }
    }

    /// 返回点击事件是否由键盘事件生成
    pub fn is_keyboard(&self) -> bool {
        match self {
            ClickEvent::Mouse(_) | ClickEvent::Touch(_) => false,
            ClickEvent::Keyboard(_) => true,
        }
    }
}

/// 表示点击事件中按下的键盘按键的枚举。
#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Default)]
pub enum KeyboardButton {
    /// 按下了 Enter 键
    #[default]
    Enter,
    /// 按下了空格键
    Space,
}

/// 表示按下的鼠标按键的枚举。
#[derive(Hash, Default, PartialEq, Eq, Copy, Clone, Debug)]
pub enum MouseButton {
    /// 鼠标左键。
    #[default]
    Left,

    /// 鼠标右键。
    Right,

    /// 鼠标中键。
    Middle,

    /// 导航按键，如前进或后退。
    Navigate(NavigationDirection),
}

impl MouseButton {
    /// 获取所有鼠标按键的列表。
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

/// 导航方向，如前进或后退。
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

    /// 按下的鼠标按键（如果有）。
    pub pressed_button: Option<MouseButton>,

    /// 鼠标移动时按住的修饰键。
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
    /// 如果鼠标左键当前被按住则返回 true。
    pub fn dragging(&self) -> bool {
        self.pressed_button == Some(MouseButton::Left)
    }
}

/// 来自平台的鼠标滚轮事件。
#[derive(Clone, Debug, Default)]
pub struct ScrollWheelEvent {
    /// 鼠标在窗口上的位置。
    pub position: Point<Pixels>,

    /// 此事件的滚轮位置变化量。
    pub delta: ScrollDelta,

    /// 鼠标移动时按住的修饰键。
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
    /// 以行为单位的近似滚动增量。
    Lines(Point<f32>),
}

impl Default for ScrollDelta {
    fn default() -> Self {
        Self::Lines(Default::default())
    }
}

/// 来自平台的捏合手势事件，当用户执行捏合缩放手势（通常在触摸板上）时生成。
///
#[derive(Clone, Debug, Default)]
pub struct PinchEvent {
    /// 捏合中心在窗口上的位置。
    pub position: Point<Pixels>,

    /// 此事件的缩放增量。
    /// 正值表示放大，负值表示缩小。
    /// 例如，0.1 表示 10% 的缩放增加。
    pub delta: f32,

    /// 捏合手势期间按住的修饰键。
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
    /// 如果这是以像素为单位的精确滚动增量则返回 true。
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
    /// 如果增量符号相同（都为正或都为负），则将增量相加。
    /// 如果符号相反，则使用第二个增量（other），有效地覆盖第一个增量。
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
    /// 按下的鼠标按键（如果有）。
    pub pressed_button: Option<MouseButton>,
    /// 鼠标移动时按住的修饰键。
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

/// 来自平台的路径集合，例如文件拖放时的路径。
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
        // the platform will render icons for the dragged files
        Empty
    }
}

/// 来自平台的文件拖放事件，当文件被拖放到窗口上时生成。
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
    /// 文件已被放置到窗口上。
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

/// 对应所有平台输入事件类型的枚举。
#[derive(Clone, Debug)]
pub enum PlatformInput {
    /// 按下了一个按键。
    KeyDown(KeyDownEvent),
    /// 释放了一个按键。
    KeyUp(KeyUpEvent),
    /// 键盘修饰键发生了变化。
    ModifiersChanged(ModifiersChangedEvent),
    /// 鼠标被按下。
    MouseDown(MouseDownEvent),
    /// 鼠标被释放。
    MouseUp(MouseUpEvent),
    /// 鼠标压力。
    MousePressure(MousePressureEvent),
    /// 鼠标被移动。
    MouseMove(MouseMoveEvent),
    /// 鼠标离开了窗口。
    MouseExited(MouseExitEvent),
    /// 使用了滚轮。
    ScrollWheel(ScrollWheelEvent),
    /// 执行了捏合手势。
    Pinch(PinchEvent),
    /// 文件被拖放到窗口上。
    FileDrop(FileDropEvent),
    /// 触摸屏上的原始触摸事件。
    Touch(TouchEvent),
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
            PlatformInput::Touch(_) => None,
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
            PlatformInput::Touch(_) => None,
        }
    }

    /// 返回此输入中包含的触摸事件（如果有）。
    pub fn touch_event(&self) -> Option<&TouchEvent> {
        match self {
            PlatformInput::Touch(event) => Some(event),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        self as rgpui, AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement,
        KeyBinding, Keystroke, Modifiers, ParentElement, Render, TestAppContext, Window, div,
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

    #[rgpui::test]
    fn test_multi_modifier_gesture_does_not_dispatch_standalone_modifier_binding(
        cx: &mut TestAppContext,
    ) {
        let (test_view, cx) = cx.add_window_view(|_, cx| TestView {
            saw_key_down: false,
            saw_action: false,
            focus_handle: cx.focus_handle(),
        });

        cx.update(|_, cx| {
            cx.bind_keys(vec![KeyBinding::new("shift", TestAction, None)]);
        });
        test_view.update_in(cx, |test_view, window, cx| {
            window.focus(&test_view.focus_handle, cx);
        });

        cx.simulate_modifiers_change(Modifiers::alt());
        cx.simulate_modifiers_change(Modifiers::alt() | Modifiers::shift());
        cx.simulate_modifiers_change(Modifiers::shift());
        cx.simulate_modifiers_change(Modifiers::none());
        assert!(!test_view.read_with(cx, |test_view, _| test_view.saw_action));

        cx.simulate_modifiers_change(Modifiers::shift());
        cx.simulate_modifiers_change(Modifiers::none());
        assert!(test_view.read_with(cx, |test_view, _| test_view.saw_action));
    }
}
