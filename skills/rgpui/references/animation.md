# 动画 / 手势 / 滚动物理（Animation/Gestures/Physics）

**目录：** [概述](#概述) · [Animation](#animation) · [动画预设与关键帧](#动画预设与关键帧) · [缓动函数](#缓动函数) · [Transition](#transition) · [Spring](#spring) · [时长常量与辅助](#时长常量与辅助) · [手势（GestureDetector）](#手势gesturedetector) · [滚动物理（ScrollPhysics）](#滚动物理scrollphysics) · [prelude 导出](#prelude-导出) · [最佳实践](#最佳实践)

## 概述

三个独立子系统：

- 动画：`crates/rgpui/src/animation/`（`pub mod animation;` `rgpui.rs:243`）+
  `elements/animation.rs`（根导出 `Animation`/`AnimationExt`/
  `AnimationElement`）+ `transition.rs`（根导出 `Transition`，`rgpui.rs:521`）。
- 手势：`mouse_gestures/mod.rs`（`pub mod mouse_gestures;` `rgpui.rs:263`）。
- 滚动物理：`scroll_physics/mod.rs`（`pub mod scroll_physics;` `rgpui.rs:266`）。

示例：`cargo run -p showcase --bin animation`。

## Animation

`Animation`（`elements/animation.rs:16`）：`new(Duration)`（`:33`）、
`repeat()`（`:44`）、`repeat_synced()`（`:50`）、
`with_easing(impl Fn(f32) -> f32 + 'static)`（`:58`）、`with_max_fps(f32)`（`:65`）。
字段：`duration`/`oneshot`/`synced`/`easing`/`max_fps`。

`AnimationExt`（`:77`）：
- `with_animation(id, Animation, |elem, f32| elem)`（`:79`）
- `with_animations(id, Vec<Animation>, |elem, usize, f32| elem)`（`:97`）

**自动遵守系统「减少动态效果」**：`cx.reduce_motion()` 为真时动画立即到终点。

`AnimationElement<E>`（`:118`）：`map_element(f)`（`:137`）。

```rust
use rgpui::prelude::*;
use rgpui::Animation;
use std::time::Duration;

div()
    .size_16()
    .with_animation("pulse", Animation::new(Duration::from_millis(800)).repeat(), |el, t| {
        el.opacity(0.3 + 0.7 * t)
    })
```

## 动画预设与关键帧

`animation/animate.rs`：

- `AnimationPreset`（`:17`）：`new(duration, easing)`（`:28`）、`duration`/`with_duration`、
  `easing`/`with_easing`、`delay`/`with_delay`（`:37`-`:70`）、getter（`:73`-`:85`）、
  `to_animation()`（`:91`）。
- 预设构造器：`fade_in()`（`:97`）、`fade_out()`（`:102`）、`slide_up()`（`:107`）、
  `slide_down()`（`:112`）、`scale_in()`（`:117`，回弹）、`bounce_in()`（`:122`，弹性）、
  `slide_in_left()`（`:127`）、`slide_in_right()`（`:132`）。
- `AnimationRepeat`（`:138`）：`Once`/`Count(u32)`/`Infinite`。
- `KeyframeAnimation`（`:149`）：`new(id)`（`:164`）、`at(pct, value)`（`:175`）、
  `duration`（`:192`）、`repeat(AnimationRepeat)`（`:198`）、`easing`（`:204`）、
  `interpolate(progress)`（`:225`）、`to_animation()`（`:255`）。
- `StaggerConfig`（`:266`）：`new()`（`:275`）、`delay_per_child(Duration)`（`:283`）、
  `animation(AnimationPreset)`（`:289`）、`delay_for_index(index)`（`:295`）、
  `preset_for_index(index)`（`:300`）。

## 缓动函数

`elements/animation.rs`：`linear`（`:286`）、`quadratic`（`:291`）、
`ease_in_out`（`:296`）、`ease_out_quint()`（`:306`，返回闭包）、
`bounce(easing)`（`:311`）、`pulsating_between(min, max)`（`:322`）。

`animation/easing.rs`：`ease_out_back`（`:24`）、`ease_in_out_back`（`:37`）、
`elastic`（`:49`）、`ease_in_elastic`（`:64`）、`ease_out_elastic`（`:77`）。

`transition.rs`：`cubic_bezier(x1,y1,x2,y2)`（`:15`）、`ease_out_cubic`（`:31`）、
`ease_in_cubic`（`:37`）、`ease_in_out_cubic`（`:43`）。

## Transition

`Transition`（`transition.rs:106`，根导出）：`new(Duration)`（`:126`）、
`ease(impl Fn(f32) -> f32 + 'static)`（`:135`）、`slide_y(from, to)`（`:141`）、
`slide_x`（`:147`）、`fade(from, to)`（`:153`）、`width(from, to)`（`:159`）、
`height`（`:165`）、`apply<E: IntoElement + Styled + 'static>(element, id)`（`:171`）。

```rust
use rgpui::Transition;
use std::time::Duration;

Transition::new(Duration::from_millis(200))
    .ease(rgpui::animation::easing::ease_out_back)
    .fade(0., 1.)
    .apply(div().child("淡入"), "fade-in")
```

`trait Lerp`（`:53`）用于值插值。

## Spring

`Spring`（`animation/spring.rs:11`）：`new(stiffness, damping, mass)`（`:40`）、
预设 `gentle()`（`:53`）/`wobbly()`（`:58`）/`stiff()`（`:63`）/`slow()`（`:68`）/
`snappy()`（`:73`）、`with_position`（`:78`）、`with_target`（`:84`）、
`with_velocity`（`:90`）、`with_rest_threshold`（`:96`）、`set_target`（`:102`）、
`set_position`（`:107`）、`impulse(velocity)`（`:112`）、`tick(dt) -> bool`（`:119`，
返回是否仍在运动）、`tick_duration(Duration) -> bool`（`:142`）、`is_at_rest()`（`:147`）、
`progress()`（`:155`）、`reset()`（`:168`）、`snap_to_target()`（`:174`）。

`SpringBridge`（`animation/bridge.rs:18`，把弹簧桥接为按时间推进的动画）：
`new(spring, duration)`（`:31`）、`from_animation(spring, &Animation)`（`:41`）、
`advance(delta) -> bool`（`:49`）、`spring()`（`:58`）、`elapsed()`（`:63`）、`is_at_rest()`（`:68`）。

## 时长常量与辅助

`animation/durations.rs`：`ULTRA_FAST`(100ms)、`FASTEST`(150ms)、`FAST`(200ms)、
`NORMAL`(300ms)、`SLOW`(400ms)、`SLOWEST`(500ms)、`EXTRA_SLOW`(600ms)。

`animation/helpers.rs`：`lerp_pixels(from, to, t)`（`:6`）、`lerp_color(from, to, t)`（`:12`）。

## 手势（GestureDetector）

`GestureDetector`（`mouse_gestures/mod.rs:89`，状态机，`Clone`）：
`new()`（`:122`）、`with_long_press_duration(Duration)`（`:139`）、
`with_swipe_distance(f32)`（`:145`）、`on_mouse_down(Point<Pixels>) -> Vec<GestureEvent>`（`:151`）、
`on_mouse_move(...)`（`:162`）、`on_mouse_up(...)`（`:198`）、`check_long_press() -> Option<GestureEvent>`（`:279`）、
`is_pressed()`（`:298`）、`is_panning()`（`:303`）、`reset()`（`:308`）。

`GestureEvent`（`:61`）：`Swipe(SwipeGesture)` / `LongPress(LongPressGesture)` /
`Tap(TapGesture)` / `PanStart(Point<Pixels>)` / `PanUpdate(PanGesture)` / `PanEnd(PanGesture)`。
`SwipeDirection`（`:8`）。
阈值常量：`SWIPE_MIN_DISTANCE=50.0`、`SWIPE_MIN_VELOCITY=200.0`（`:77-79`）、
`LONG_PRESS_DURATION=500ms`、`DOUBLE_TAP_INTERVAL=300ms`、`PAN_THRESHOLD=5.0`（`:81-85`）。

## 滚动物理（ScrollPhysics）

`ScrollPhysics`（`scroll_physics/mod.rs:8`）：`new()`（`:35`）、
`with_bounds(min, max)`（`:49`）、`with_deceleration(f32)`（`:56`）、
`with_overscroll_resistance(f32)`（`:62`）、`momentum(bool)`（`:68`）、`overscroll(bool)`（`:74`）、
`set_bounds`（`:80`）、`apply_delta(f32)`（`:86`）、`tick(dt) -> bool`（`:102`）、
`position()`（`:143`）、`velocity()`（`:148`）、`is_moving()`（`:153`）、
`is_overscrolled()`（`:158`）、`stop()`（`:163`）、`reset()`（`:168`）、
`set_position`（`:174`）、`scroll_to`（`:179`）、`fling(velocity)`（`:185`）。

## prelude 导出

`prelude.rs` 已导出（`:25`-`:35`）：`AnimationPreset`、`AnimationRepeat`、
`KeyframeAnimation`、`Spring`、`SpringBridge`、`StaggerConfig`、`bounce_in`、`fade_in`、
`fade_out`、`scale_in`、`slide_down`、`slide_in_left`、`slide_in_right`、`slide_up`、
`GestureDetector`、`GestureEvent`、`LongPressGesture`、`PanGesture`、`SwipeDirection`、
`SwipeGesture`、`TapGesture`、`ScrollPhysics`。

**核心 `Animation` 类型不在 prelude**：用 `rgpui::Animation`（根导出）；
`Transition` 用 `rgpui::Transition`。**时长常量不 glob**，需显式
`use rgpui::animation::durations::*`。

## 最佳实践

- **动画默认尊重减少动态效果**（`reduce_motion`）：无需手动判断。
- **简单过渡用 `Transition`**，循环/多段用 `Animation` + `with_animation(s)`。
- **物理回弹**用 `ScrollPhysics`，滚动容器内建越界回弹见 `scroll.md`。
- **手势用手势检测器状态机**（自行喂鼠标事件），或用 `mouse_gestures` 之外的
  `elements` 事件回调（`on_mouse_down` 等）实现简单交互。
