# 动画时间线（Anim Timeline）

**目录：** [概述](#概述) · [数据模型](#数据模型) · [Interpolation](#interpolation) · [几何自由函数](#几何自由函数) · [控件 TimelineState](#控件-timelinestate) · [事件与命中](#事件与命中) · [TimelineView](#timelineview) · [最佳实践](#最佳实践)

## 概述

`crates/rgpui/src/anim_timeline.rs`，`pub mod anim_timeline;`（`rgpui.rs:301`）——
**核心，无 feature 门控，不在 prelude**，路径 `rgpui::anim_timeline::*`。多轨关键帧编辑：
时间标尺、轨道、关键帧、可拖拽播放头、按插值绘制的值曲线。分两层：纯数据模型
（`Timeline`/`Track`/`Keyframe`/`Interpolation`）负责存储与求值，控件
（`TimelineState`/`TimelineView`）负责命中、拖拽（水平改时间、垂直改值）与播放。

示例：`cargo run -p showcase --bin timeline`（`examples/showcase/src/bin/timeline.rs`）。

## 数据模型

`Keyframe`（`:77`）：`{ pub time: f32, pub value: f32 }`；`new(time, value)`（`:87`，时间夹为非负）。

`Track`（`:147`）：字段 `pub name`/`pub keyframes`/`pub interpolation`/`pub collapsed`。
构造：`new(name)`（`:161`）、`with_keyframes(name, Vec<Keyframe>)`（`:171`，自动排序去重）、
`with_interpolation`（`:179`）、`with_collapsed`（`:185`）。
关键帧操作：`set_keyframe(time, value) -> bool`（`:237`，同时间覆盖返回 false、新增 true）、
`remove_keyframe(index) -> Option<Keyframe>`（`:253`）、`keyframe_index_at(time, tolerance) -> Option<usize>`
（`:262`，容差内最近帧下标）、`move_keyframe(index, time) -> Option<usize>`（`:277`，改时间重排、返回新下标）、
`set_keyframe_value(index, value) -> bool`（`:294`，只改值不重排）。
求值/折叠：`normalize()`（`:219`，排序+去重）、`value_at(time) -> f32`（`:339`，首末钳制）、
`value_range() -> (f32, f32)`（`:312`，上下各留 10%）、`duration() -> f32`（`:334`，末帧时间）、
`set_collapsed(bool) -> bool`（`:191`）、`row_height() -> Pixels`（`:200`，折叠=`COLLAPSED_TRACK_HEIGHT`）、
`to_keyframe_animation(id, duration) -> KeyframeAnimation`（`:367`，绝对秒→进度 0~1）。

`Timeline`（`:382`）：`new()`（`:390`）、`from_tracks(Vec<Track>)`（`:395`）、
`tracks()/tracks_mut()`（`:400`/`:405`）、`track(i)/track_mut(i)`（`:410`/`:415`）、
`add_track(Track) -> usize`（`:420`）、`remove_track(i) -> Option<Track>`（`:426`）、
`move_track(from, to) -> bool`（`:435`）、`len()`/`is_empty()`（`:449`/`:454`）、
`duration() -> f32`（`:459`，各轨最大值）、`values_at(time) -> Vec<f32>`（`:464`）。

## Interpolation

`enum Interpolation`（`:96`）6 变体：`Linear`（默认，`:100`）、`Hold`（阶跃）、`Smooth`
（smoothstep）、`EaseIn`（二次缓入）、`EaseOut`（二次缓出）、`EaseInOut`（三次）。
`ease(self, t) -> f32`（`:115`，局部进度→系数）、`label(self) -> &'static str`（`:134`，中文名）。
> 注：模块 `//!` 头注释只写了「线性 / 保持 / 平滑」三种，源码实为 6 种（另有缓入/缓出/缓入缓出）。

## 几何自由函数

模块级 `pub fn`（`:472` 起）：`time_to_x(time, px_per_second, scroll) -> f32`（`:473`）、
`x_to_time(x, px_per_second, scroll) -> f32`（`:478`）、`snap_time(time, fps) -> f32`（`:487`，
`fps<=0` 不吸附）、`clamp_px_per_second(v) -> f32`（`:496`）、`ruler_step(px_per_second, min_px) -> f32`
（`:505`，整齐刻度步长）、`format_time(time) -> String`（`:521`，<60s 用 `1.20s` 否则 `1:02.5`）、
`value_to_y(value, range, height) -> f32`（`:533`）、`y_to_value(y, range, height) -> f32`（`:551`，
`value_to_y` 逆映射）、`curve_points(track, range, px_per_second, scroll, height) -> Vec<Point<Pixels>>`
（`:571`，按插值采样值曲线，`Hold` 得竖直跳变）。
常量：`RULER_HEIGHT`/`TRACK_HEIGHT`/`COLLAPSED_TRACK_HEIGHT`/`LABEL_WIDTH`/`DEFAULT_PX_PER_SECOND`/
`MIN_PX_PER_SECOND`/`MAX_PX_PER_SECOND`/`DEFAULT_FPS`（`:42`-`:60`）。

## 控件 TimelineState

`TimelineState::new(timeline, cx)`（`:791`）；`timeline()/timeline_mut()`（`:815`/`:820`）。
`EventEmitter<TimelineEvent>` + `Focusable` + `Render`（`:1705`-`:1713`）。

| 类别 | 方法（`cx: &mut Context<Self>`） | 位置 |
|------|------|------|
| 播放头 | `time()` / `set_time(f, cx)`（吸附+抛 `TimeChanged`） / `advance(dt, cx)`（循环回绕/停末尾） | `:825`/`:835`/`:847` |
| 播放 | `is_playing`/`play`/`pause`/`toggle_play`（`:871`-`:894`）；`is_looping`/`set_looping`（`:903`/`:908`） | — |
| 缩放 | `px_per_second()` / `set_px_per_second(f, cx)` / `zoom_by(factor, cx)` | `:916`/`:921`/`:930` |
| 滚动 | `scroll()`/`set_scroll(f,cx)`；`vscroll()`/`set_vscroll(f,cx)`/`max_vscroll()` | `:935`-`:974` |
| 轨道 | `tracks_height()`(`:977`)、`track_top(i)`(`:986`)、`scroll_to_track(i,cx)`(`:996`)、`move_track(from,to,cx)`(`:1057`)、`toggle_track_collapsed(i,cx)`(`:1018`)、`set_track_interpolation(i,interp,cx)`(`:1035`) | — |
| 吸附 | `fps()`/`set_fps(f,cx)`/`is_snapping()`/`snapped(time)` | `:1084`-`:1107` |
| 选择 | `select_keyframe(track,index,cx)`(`:1117`)、`toggle_keyframe_selection(track,index,cx)`(`:1140`，Ctrl/Cmd)、`selection()`(`:1112`)、`selected_keyframe_indices()`(`:1174`)、`is_keyframe_selected(t,i)`(`:1186`)、`clear_selection(cx)`(`:1260`) | — |
| 删除 | `remove_selected_keyframe(cx)`（单个,`:1265`）/`remove_selected_keyframes(cx)->usize`（全部,`:1194`） | — |
| 框选 | `box_selecting() -> Option<(usize, Point, Point)>`（`:1226`，轨道+锚点+当前位置） | — |
| 命中/聚焦 | `hit_test(local) -> TimelineHit`（`:1287`）、`focus(window,cx)`（`:1282`） | — |

播放推进由应用每帧调 `advance`，控件本身不持定时器。

## 事件与命中

`enum TimelineHit`（`:657`）：`Ruler`、`Keyframe { track, index }`、`Track { track }`、`None`。

`enum TimelineEvent`（`:678`）：`TimeChanged(f32)`、`SelectionChanged { track: Option<usize>,
keyframe: Option<usize> }`、`KeyframeMoved { track, index, time, value }`（`index` 为移动后下标）、
`KeyframeRequested { track, time }`（双击空白）、`KeyframeRemoved { track, index }`、
`PlaybackChanged(bool)`、`TrackMoved { from, to }`、`TrackCollapsed { index, collapsed }`、
`TrackInterpolation { index, interpolation }`。

交互（`on_pointer_down/move/up`、`on_wheel`、`on_key_down`，均私有）：拖标尺移播放头；拖关键帧
水平改时间、垂直改值（拖期间冻结 `drag_range` 防抖动）；双击轨道空白请求新增；轨道空白按下
拖动=框选；`Ctrl/Cmd+滚轮`缩放（保持指针下时间不动）、`Shift+滚轮`纵向滚动、其余滚轮平移时间轴；
键盘 `Space` 播放/暂停、方向键移播放头、`Home/End`、`Delete` 删选中、`±` 缩放。

## TimelineView

`TimelineView`（`:1838`，`#[derive(IntoElement)]`）：`new(&Entity<TimelineState>)`（`:1844`）。
薄包装：`key_context("AnimTimeline")` + `track_focus` + 挂 key/鼠标按下/移动/滚轮/
`capture_any_mouse_up`，内部渲染 `TimelineState`。

## 最佳实践

- **模型与控件分离**：编辑数据用 `Timeline`/`Track`；播放头、缩放、选中、命中都走 `TimelineState`。
- **改模型后要 `cx.notify()`**（`timeline_mut()` 返回可变引用不自动通知）。
- **播放靠应用驱动**：定时器每帧 `advance(dt, cx)`；循环/停止由 `set_looping` 决定。
- **编辑完即可播**：`Track::to_keyframe_animation(id, duration)` 转成 `rgpui::animation::KeyframeAnimation`。
- **几何/吸附是自由函数**：`time_to_x`/`x_to_time`/`snap_time`/`value_to_y`/`y_to_value`/
  `ruler_step`/`format_time`/`curve_points` 从 `rgpui::anim_timeline::` 直接导入。
